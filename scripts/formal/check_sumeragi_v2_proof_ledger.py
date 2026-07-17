#!/usr/bin/env python3
"""Validate the Sumeragi v2 proof ledger and reject unchecked proof escapes."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from collections.abc import Sequence
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path
from typing import Any


ROOT_DIR = Path(__file__).resolve().parents[2]
FORMAL_DIR = ROOT_DIR / "docs" / "formal" / "sumeragi_v2"
LEDGER_PATH = FORMAL_DIR / "proof_coverage.json"
VERUS_SOURCE_DIR = ROOT_DIR / "crates" / "iroha_sumeragi_core" / "src"
TLAPM_COMMIT = "763bf3c1826d77a4cf206f43d5aa16775da1da33"
EVIDENCE_SCHEMA_VERSION = 1

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
    "SumeragiV2VocabularyProofs",
    "SumeragiV2SafetyLemmas",
    "SumeragiV2AgreementLemmas",
    "SumeragiV2ChainEpochProofs",
    "SumeragiV2InductiveProofs",
    "SumeragiV2Proofs",
    "SumeragiV2TimeoutDurability",
    "SumeragiV2TimeoutSigningInvariant",
    "SumeragiV2TimeoutViewInvariant",
    "SumeragiV2TimeoutWireAuthorization",
    "SumeragiV2ChainEpochRefinement",
    "SumeragiV2SuccessorActivationRefinementProofs",
    "SumeragiV2TemporalLemmas",
    "SumeragiV2LivenessProofs",
    "SumeragiV2ServiceRankLemmas",
    "SumeragiV2AsyncLivenessProofs",
    "SumeragiTimeoutIngressGuardTest",
)

REQUIRED_MODEL_MODULES = (
    "SumeragiV2",
    "SumeragiV2Quorums",
    "SumeragiV2QuorumProofs",
    "SumeragiV2Availability",
    "SumeragiV2Core",
    "SumeragiV2ResumeVoteWitness",
    "SumeragiV2CrashRecovery",
    "SumeragiV2Reconfiguration",
    "SumeragiV2VocabularyProofs",
    "SumeragiV2SafetyDefinitions",
    "SumeragiV2SafetyLemmas",
    "SumeragiV2AgreementLemmas",
    "SumeragiV2Inductive",
    "SumeragiV2InductiveProofs",
    "SumeragiV2Proofs",
    "SumeragiV2TimeoutDurability",
    "SumeragiV2TimeoutSigningInvariant",
    "SumeragiV2TimeoutViewInvariant",
    "SumeragiV2TimeoutWireAuthorization",
    "SumeragiV2ChainEpoch",
    "SumeragiV2ChainEpochProofs",
    "SumeragiV2ChainEpochRefinement",
    "SumeragiV2SuccessorActivationRefinementProofs",
    "SumeragiV2TemporalLemmas",
    "SumeragiV2LivenessProofs",
    "SumeragiV2ServiceRankLemmas",
    "SumeragiV2AsyncNetwork",
    "SumeragiV2AsyncLivenessProofs",
    "SumeragiTimeoutIngressGuardTest",
)

REQUIRED_TLC_CONFIGS = (
    "quorum_count.cfg",
    "quorum_stake.cfg",
    "safety_count.cfg",
    "safety_stake.cfg",
    "chain_epoch.cfg",
    "liveness.cfg",
    "resume_locked_commit_witness.cfg",
)

REQUIRED_TLC_CONFIG_HEADERS = {
    "quorum_count.cfg": "INIT Init\nNEXT QuorumCheckNext",
    "quorum_stake.cfg": "INIT Init\nNEXT QuorumCheckNext",
    "safety_count.cfg": "INIT Init\nNEXT Next",
    "safety_stake.cfg": "INIT Init\nNEXT Next",
    "chain_epoch.cfg": "SPECIFICATION ChainEpochTlcSpec",
    "liveness.cfg": "SPECIFICATION AsyncFiniteSpec",
    "resume_locked_commit_witness.cfg": "SPECIFICATION CoreSpec",
}

RETIRED_PATHS = (
    ROOT_DIR / "docs" / "formal" / "sumeragi",
    ROOT_DIR / "scripts" / "formal" / "sumeragi_apalache.sh",
    ROOT_DIR / "scripts" / "formal" / "sumeragi_tlc.sh",
    ROOT_DIR / "scripts" / "formal" / "check_sumeragi_formal_coverage.py",
    ROOT_DIR / "ci" / "check_sumeragi_formal_expected_failures.sh",
    ROOT_DIR / "pytests" / "scripts" / "sumeragi_formal_coverage_test.py",
)

# The first-release proof models the scheduler and transport explicitly in
# AsyncSpec.  These names belonged to the former favourable-network corridor,
# which encoded the desired progress steps directly into a second protocol
# relation and could therefore make a circular liveness claim look proved.
RETIRED_LIVENESS_SYMBOLS = (
    "ReliableBeginTimeout",
    "ReliableNext",
    "ReliableNextV2",
    "ReliableActionFairness",
    "LivenessSpec",
    "StableProgressContracts",
)

# These predicates mention proof-only global history.  They may be used in
# inductive lemmas, but never as executable guards on the protocol actions
# whose provenance the proof is supposed to derive.
REACHABLE_ACTION_ORACLES = {
    "FormPrepareQC": ("CertificateHonestIntentBacked", "QcValid"),
    "FormCommitQC": ("CertificateHonestIntentBacked", "QcValid"),
    "DeliverQC": ("CertificateHonestIntentBacked", "QcValid"),
    "BeginTimeout": ("HighRefValid", "CertificateHonestIntentBacked"),
}

# Release safety is proved for one arbitrary frozen height context.  The old
# genesis-only ``Spec``/``NextV2`` wrappers include a global application
# barrier and therefore cannot discharge any of these obligations.
ARBITRARY_CONTEXT_SAFETY_OBLIGATIONS = {
    "durable-vote-uniqueness": "DurableVoteUniquenessObligation",
    "lock-monotonicity": "LockMonotonicityObligation",
    "external-validity": "ExternalValidityObligation",
    "certified-body-availability": "AvailabilityObligation",
    "certificate-uniqueness": "CertificateUniquenessObligation",
    "timeout-protection": "TimeoutProtectionObligation",
    "agreement": "AgreementObligation",
    "no-conflicting-commit-qcs": "NoConflictingCommitCertificatesObligation",
    "crash-restart": "CrashRecoveryObligation",
}
ARBITRARY_CONTEXT_SAFETY_PROPERTY_WRAPPERS = {
    "durable-vote-uniqueness": "DurableVoteUniquenessProperty",
    "lock-monotonicity": "LockMonotonicityProperty",
    "external-validity": "ExternalValidityProperty",
    "certified-body-availability": "CertifiedBodyAvailabilityProperty",
    "certificate-uniqueness": "CertificateUniquenessProperty",
    "timeout-protection": "TimeoutProtectionProperty",
    "agreement": "AgreementProperty",
    "no-conflicting-commit-qcs": "NoConflictingCommitCertificatesProperty",
    "crash-restart": "CrashRecoveryProperty",
}

# These are properties of the concrete asynchronous scheduler and transport,
# not wrappers that may be stated in an upstream safety module.
ASYNC_LIVENESS_OBLIGATIONS = {
    "generation-scoped-vote-delivery": "GenerationScopedVoteDeliveryObligation",
    "progress-witness-preservation": "ProgressWitnessObligation",
    "post-gst-deadlock-freedom": "DeadlockFreedomObligation",
    "protected-service-rank": "ProtectedServiceRankProgressObligation",
    "post-gst-starvation-freedom": "StarvationFreedomObligation",
    "timeout-view-liveness": "TimeoutViewProgressObligation",
    "rotating-leader-liveness": "RotatingLeaderProgressObligation",
    "application-liveness": "ApplicationLivenessObligation",
}
ASYNC_LIVENESS_PROPERTY_WRAPPERS = {
    "generation-scoped-vote-delivery": "GenerationScopedVoteDeliveryProperty",
    "progress-witness-preservation": "ProgressWitnessProperty",
    "post-gst-deadlock-freedom": "DeadlockFreedomProperty",
    "protected-service-rank": "ProtectedServiceRankProgressProperty",
    "post-gst-starvation-freedom": "StarvationFreedomProperty",
    "timeout-view-liveness": "TimeoutViewProgressProperty",
    "rotating-leader-liveness": "RotatingLeaderProgressProperty",
    "application-liveness": "ApplicationLivenessProperty",
}

# These obligations are release-architecture seams, not declarations that may
# drift between proof modules.  Type closure belongs to the concrete async
# proof; successor/catch-up production refinement, genesis handoff, and
# multi-height progress belong to the current receipt-driven indexed chain
# product.
FIXED_PROOF_OBLIGATION_TARGETS = {
    "timeout-wire-authorization": (
        "SumeragiV2TimeoutWireAuthorization",
        "CoreSpecAtAlwaysStrongTimeoutWireAuthorizationInvariant / "
        "StrongWireInvariantAuthorizesPendingTimeoutSignature / "
        "StrongWireInvariantAuthorizesHonestTimeoutEnvelope",
    ),
    "historical-tc-lock-commit": (
        "SumeragiV2Proofs",
        "HistoricalTcLockedCommitAuthorizationObligation",
    ),
    "effective-lock-body-acquisition": (
        "SumeragiV2AsyncLivenessProofs",
        "EffectiveLockBodyAcquisitionCompositionObligation",
    ),
    "async-type-invariant": (
        "SumeragiV2AsyncLivenessProofs",
        "AsyncTypeInvariantObligation",
    ),
    "successor-activation-catch-up-production-refinement": (
        "SumeragiV2ChainEpochRefinement",
        "SuccessorActivationAndHistoricalCatchUpProductionRefinementObligation",
    ),
    "genesis-height-successor-handoff": (
        "SumeragiV2ChainEpochRefinement",
        "GenesisHeightSuccessorHandoffObligation",
    ),
    "height-liveness": (
        "SumeragiV2ChainEpochRefinement",
        "HeightLivenessObligation",
    ),
}

# Temporal composition may be promoted only after its proof dependencies.  The
# ledger order is intentional as well: reviewers should encounter each
# prerequisite before the theorem which consumes it.
PROOF_STATUS_DEPENDENCIES = {
    "timeout-protection": ("historical-tc-lock-commit",),
    "async-type-invariant": ("async-runner-scheduler-preservation",),
    "progress-witness-preservation": (
        "async-type-invariant",
        "generation-scoped-vote-delivery",
    ),
    "post-gst-deadlock-freedom": ("async-type-invariant",),
    "protected-service-rank": ("async-type-invariant",),
    "post-gst-starvation-freedom": (
        "async-type-invariant",
        "protected-service-rank",
    ),
    "timeout-view-liveness": (
        "progress-witness-preservation",
        "post-gst-starvation-freedom",
    ),
    "rotating-leader-liveness": (
        "effective-lock-body-acquisition",
        "progress-witness-preservation",
        "post-gst-starvation-freedom",
        "timeout-view-liveness",
    ),
    "application-liveness": (
        "progress-witness-preservation",
        "post-gst-starvation-freedom",
    ),
    "genesis-height-successor-handoff": (
        "rotating-leader-liveness",
        "application-liveness",
        "successor-activation-catch-up-production-refinement",
    ),
    "height-liveness": (
        "rotating-leader-liveness",
        "application-liveness",
        "successor-activation-catch-up-production-refinement",
    ),
}

# Multi-height safety belongs to the receipt-driven, per-node chain model.
# Binding these obligations to the one-height Core theorem module would prove
# only a fixed context and could silently reintroduce the retired global apply
# barrier through the old reconfiguration wrapper.
CHAIN_SAFETY_OBLIGATIONS = {
    "chain-prefix": ("ChainPrefixObligation", "ChainPrefixProperty"),
    "epoch-boundary": ("EpochBoundaryObligation", "EpochBoundaryProperty"),
}

MODULE_HEADER_RE = re.compile(r"(?m)^---- MODULE ([A-Za-z_][A-Za-z0-9_]*) ----$")
DECLARATION_TEMPLATE = r"(?m)^{symbol}\s*(?:\([^)=\n]*\))?\s*=="
THEOREM_DECLARATION_TEMPLATE = (
    r"(?m)^[ \t]*(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)[ \t]+"
    r"{symbol}\s*(?:\([^)=\n]*\))?\s*=="
)
ANY_THEOREM_DECLARATION_RE = re.compile(
    r"(?m)^[ \t]*(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)[ \t]+"
    r"[A-Za-z_][A-Za-z0-9_]*\s*(?:\([^)=\n]*\))?\s*=="
)
TOP_LEVEL_TRUST_RE = re.compile(
    r"(?mi)^[ \t]*(?P<token>ASSUME(?:S)?|ASSUMPTION(?:S)?|AXIOM(?:S)?)\b"
)
THEOREM_PROOF_MARKER_RE = re.compile(r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b")
STRUCTURED_PROVE_RE = re.compile(r"(?mi)^[ \t]*PROVE\b")
OMITTED_RE = re.compile(r"(?i)\bOMITTED\b")
VERUS_ESCAPE_RE = re.compile(
    r"(?i)(?:\b(?:assume|admit)\s*!?\s*\(|"
    r"#\s*\[\s*verifier\s*::\s*(?:external_body|external_fn_specification|"
    r"external_type_specification|assume_specification|trusted)\s*\])"
)
TLAPM_COMPLETE_RE = re.compile(
    r"\[INFO\]: All ([1-9][0-9]*) obligation(?:s)? proved\."
)
TLAPM_RUNNER_MARKER_PREFIX = "SUMERAGI_TLAPS_BACKEND_COMPLETE"


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


def load_ledger(path: Path = LEDGER_PATH) -> Any:
    """Load a ledger while rejecting duplicate JSON keys."""

    return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=_unique_object)


# One validation pass revisits every formal module through several independent
# fail-closed checks.  Keep the cache larger than the corridor's module count
# so those checks stay linear without retaining an unbounded number of mutated
# test fixtures.
@lru_cache(maxsize=64)
def strip_tla_comments(
    source: str, *, preserve_string_contents: bool = False
) -> str:
    """Remove TLA+ comments while preserving lines and, optionally, strings."""

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
                output.extend(pair if preserve_string_contents else "  ")
                index += 2
                continue
            if char == '\"':
                output.append(char)
                in_string = False
            else:
                output.append(
                    char
                    if preserve_string_contents
                    else ("\n" if char == "\n" else " ")
                )
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
    structured_assumptions: set[int] = set()
    for declaration in ANY_THEOREM_DECLARATION_RE.finditer(stripped):
        body_start = declaration.end()
        next_declaration = re.compile(
            r"(?m)^(?:[A-Za-z_][A-Za-z0-9_]*\s*"
            r"(?:\([^)=\n]*\))?\s*==|[ \t]*(?:LOCAL[ \t]+)?"
            r"(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)\b|={4,}\s*$)"
        ).search(stripped, body_start)
        body_end = (
            next_declaration.start()
            if next_declaration is not None
            else len(stripped)
        )
        body = stripped[body_start:body_end]
        proof = THEOREM_PROOF_MARKER_RE.search(body)
        if proof is None:
            continue
        statement = body[: proof.start()]
        assumption = re.match(
            r"(?is)\s*(?P<token>ASSUME)\b",
            statement,
        )
        if assumption is None:
            continue
        if STRUCTURED_PROVE_RE.search(statement, assumption.end()) is None:
            continue
        structured_assumptions.add(body_start + assumption.start("token"))

    errors: list[str] = []
    for match in TOP_LEVEL_TRUST_RE.finditer(stripped):
        if match.start("token") in structured_assumptions:
            continue
        line = stripped.count("\n", 0, match.start()) + 1
        token = match.group("token")
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


def _symbol_exists(module_source: str, symbol: str, *, theorem_only: bool = False) -> bool:
    """Return whether ``symbol`` has the required top-level declaration shape."""

    stripped = strip_tla_comments(module_source)
    theorem_pattern = re.compile(
        THEOREM_DECLARATION_TEMPLATE.format(symbol=re.escape(symbol))
    )
    if theorem_only:
        return theorem_pattern.search(stripped) is not None
    operator_pattern = re.compile(DECLARATION_TEMPLATE.format(symbol=re.escape(symbol)))
    return (
        operator_pattern.search(stripped) is not None
        or theorem_pattern.search(stripped) is not None
    )


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _relative_to_root(path: Path, root_dir: Path = ROOT_DIR) -> str:
    try:
        return path.resolve().relative_to(root_dir.resolve()).as_posix()
    except ValueError as error:
        raise ValueError(f"path is outside the repository: {path}") from error


def _formal_source_manifest(
    formal_dir: Path = FORMAL_DIR, root_dir: Path = ROOT_DIR
) -> dict[str, Any]:
    """Hash every TLA+ model/proof source in one deterministic manifest."""

    files: list[dict[str, str]] = []
    aggregate = hashlib.sha256()
    for path in sorted(formal_dir.glob("*.tla")):
        relative = _relative_to_root(path, root_dir)
        digest = _sha256_file(path)
        files.append({"path": relative, "sha256": digest})
        aggregate.update(relative.encode("utf-8"))
        aggregate.update(b"\0")
        aggregate.update(digest.encode("ascii"))
        aggregate.update(b"\n")
    return {"sha256": aggregate.hexdigest(), "files": files}


def _tlapm_runner_marker(module: str, source_manifest_sha256: str) -> str:
    """Return the exact marker appended only after a strict TLAPM run succeeds."""

    return (
        f"{TLAPM_RUNNER_MARKER_PREFIX} module={module} commit={TLAPM_COMMIT} "
        f"source_manifest_sha256={source_manifest_sha256}"
    )


def _tlapm_obligation_count(
    log_source: str, *, module: str, source_manifest_sha256: str
) -> int | None:
    """Validate a pinned TLAPM log and return its exact proved count.

    TLAPM 1.6.0-pre emits one final ``[INFO]: All N obligation(s) proved.``
    line on success.  The repository runner appends one manifest-bound marker
    after that line.  Requiring this exact two-line suffix prevents a stale,
    partial, or marker-stuffed log from becoming release evidence.
    """

    if not log_source.endswith("\n"):
        return None
    lines = log_source.splitlines()
    expected_marker = _tlapm_runner_marker(module, source_manifest_sha256)
    if len(lines) < 2 or lines[-1] != expected_marker:
        return None
    if sum(line.startswith(TLAPM_RUNNER_MARKER_PREFIX) for line in lines) != 1:
        return None
    completion_lines = [
        match
        for line in lines
        if (match := TLAPM_COMPLETE_RE.fullmatch(line)) is not None
    ]
    if len(completion_lines) != 1:
        return None
    completion = TLAPM_COMPLETE_RE.fullmatch(lines[-2])
    if completion is None:
        return None
    return int(completion.group(1))


def build_release_evidence(
    *,
    tlapm_version: str,
    log_dir: Path,
    formal_dir: Path = FORMAL_DIR,
    root_dir: Path = ROOT_DIR,
) -> dict[str, Any]:
    """Build source- and log-bound evidence after a successful strict TLAPM run."""

    version = " ".join(tlapm_version.split())
    if version != TLAPM_COMMIT[:7]:
        raise ValueError(
            f"TLAPM version must equal pinned identity {TLAPM_COMMIT[:7]}, found {version!r}"
        )
    source_manifest = _formal_source_manifest(formal_dir, root_dir)
    source_manifest_sha256 = source_manifest["sha256"]
    modules: list[dict[str, Any]] = []
    for module in RELEASE_PROOF_MODULES:
        log_path = log_dir / f"{module}.log"
        if not log_path.is_file() or log_path.is_symlink():
            raise ValueError(f"missing regular TLAPM proof log: {log_path}")
        source = log_path.read_text(encoding="utf-8")
        count = _tlapm_obligation_count(
            source,
            module=module,
            source_manifest_sha256=source_manifest_sha256,
        )
        if count is None or count <= 0:
            raise ValueError(
                "TLAPM proof log lacks the exact manifest-bound successful "
                f"suffix: {log_path}"
            )
        modules.append(
            {
                "module": module,
                "obligations_proved": count,
                "log": _relative_to_root(log_path, root_dir),
                "log_sha256": _sha256_file(log_path),
                "source_manifest_sha256": source_manifest_sha256,
            }
        )
    return {
        "schema_version": EVIDENCE_SCHEMA_VERSION,
        "protocol": "sumeragi-v2",
        "backend_verification": True,
        "tool": {
            "name": "TLAPM",
            "commit": TLAPM_COMMIT,
            "version": version,
        },
        "source_manifest": source_manifest,
        "modules": modules,
    }


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
        if module in RELEASE_PROOF_MODULES and not ANY_THEOREM_DECLARATION_RE.search(
            strip_tla_comments(source)
        ):
            errors.append(f"{path}: release proof module must declare a theorem")
        sources[module] = source
    return sources, errors


def _resume_vote_witness_errors(formal_dir: Path) -> list[str]:
    """Pin the bounded historical locked-Commit counterexample witness."""

    errors: list[str] = []
    module_path = formal_dir / "SumeragiV2ResumeVoteWitness.tla"
    cfg_path = formal_dir / "resume_locked_commit_witness.cfg"
    if not module_path.is_file() or not cfg_path.is_file():
        return errors

    module_source = module_path.read_text(encoding="utf-8")
    recovered = _top_level_operator_body(
        module_source,
        "RecoveredHistoricalLockedCommitSigning",
        preserve_string_contents=True,
    )
    if recovered is None:
        errors.append(
            f"{module_path}: missing historical locked-Commit recovery predicate"
        )
    else:
        body, line = recovered
        normalized = " ".join(body.split())
        required = (
            "request \\in signVotes",
            "request.vote.signer = request.node",
            "request.vote.context = context",
            'request.vote.phase = "Commit"',
            "request.vote \\in commitIntents",
            "NodeTimedOut(request.node, request.vote.view)",
            "request.vote.view < nodeView[request.node]",
            "LockedPrepareRound(request.node, request.vote.view, request.vote.subject)",
            "generation[request.node] = MaxGeneration",
        )
        missing = [token for token in required if token not in normalized]
        if missing:
            errors.append(
                f"{module_path}:{line}: recovery predicate does not require the "
                f"exact timed-out historical locked Commit after restart; missing {missing}"
            )

    negated = _top_level_operator_body(
        module_source, "NoRecoveredHistoricalLockedCommitSigning"
    )
    if negated is None:
        errors.append(f"{module_path}: missing deliberately negated witness predicate")
    else:
        body, line = negated
        if " ".join(body.split()) != "~RecoveredHistoricalLockedCommitSigning":
            errors.append(
                f"{module_path}:{line}: witness invariant must be exactly the "
                "negation of RecoveredHistoricalLockedCommitSigning"
            )

    cfg_source = cfg_path.read_text(encoding="utf-8")
    required_cfg_lines = (
        "CHECK_DEADLOCK FALSE",
        "INVARIANT TypeInvariant",
        "INVARIANT NoRecoveredHistoricalLockedCommitSigning",
        "  N = 1",
        "  Honest = {0}",
        "  Responsive = {0}",
        "  MaxHeight = 0",
        "  MaxView = 1",
        "  MaxGeneration = 2",
    )
    missing_cfg_lines = [
        line
        for line in required_cfg_lines
        if cfg_source.splitlines().count(line) != 1
    ]
    if missing_cfg_lines:
        errors.append(
            f"{cfg_path}: recovery witness configuration must pin one timed-out "
            "view advance and one restart; missing or duplicated "
            f"{missing_cfg_lines}"
        )
    return errors


def _retired_liveness_errors(formal_dir: Path) -> list[str]:
    """Reject the old favourable-network liveness shortcut by exact symbol."""

    errors: list[str] = []
    for path in sorted(formal_dir.glob("*.tla")):
        stripped = strip_tla_comments(path.read_text(encoding="utf-8"))
        for symbol in RETIRED_LIVENESS_SYMBOLS:
            for match in re.finditer(rf"\b{re.escape(symbol)}\b", stripped):
                line = stripped.count("\n", 0, match.start()) + 1
                errors.append(
                    f"{path}:{line}: retired favourable-network liveness "
                    f"symbol {symbol} is prohibited"
                )
    return errors


def _bounded_view_dependency_errors(formal_dir: Path) -> list[str]:
    """Keep ``MaxView`` confined to the finite TLC substitution scaffold."""

    errors: list[str] = []
    allowed_core_lines = {"MaxView,", "FiniteViews == 0..MaxView"}
    for path in sorted(formal_dir.glob("*.tla")):
        stripped = strip_tla_comments(path.read_text(encoding="utf-8"))
        for line_number, line in enumerate(stripped.splitlines(), start=1):
            if "MaxView" not in line:
                continue
            if path.name == "SumeragiV2Core.tla" and line.strip() in allowed_core_lines:
                continue
            errors.append(
                f"{path}:{line_number}: MaxView is reserved for FiniteViews/TLC "
                "scaffolding; deductive protocol relations must use ViewDomain"
            )
    return errors


def _top_level_operator_body(
    source: str, symbol: str, *, preserve_string_contents: bool = False
) -> tuple[str, int] | None:
    """Return one top-level operator body and its first source line."""

    stripped = strip_tla_comments(
        source, preserve_string_contents=preserve_string_contents
    )
    declaration = re.compile(
        rf"(?m)^{re.escape(symbol)}\s*(?:\([^)=]*\))?\s*=="
    ).search(stripped)
    if declaration is None:
        return None
    body_start = declaration.end()
    next_declaration = re.compile(
        r"(?m)^(?:[A-Za-z_][A-Za-z0-9_]*\s*(?:\([^)=]*\))?\s*==|"
        r"[ \t]*(?:LOCAL[ \t]+)?"
        r"(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)\b|={4,}\s*$)"
    ).search(stripped, body_start)
    body_end = next_declaration.start() if next_declaration is not None else len(stripped)
    return stripped[body_start:body_end], stripped.count("\n", 0, body_start) + 1


def _top_level_theorem_body(
    source: str, symbol: str, *, preserve_string_contents: bool = False
) -> tuple[str, int] | None:
    """Return one top-level theorem body and its first source line."""

    stripped = strip_tla_comments(
        source, preserve_string_contents=preserve_string_contents
    )
    declaration = re.compile(
        rf"(?m)^[ \t]*(?:LOCAL[ \t]+)?"
        rf"(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)[ \t]+"
        rf"{re.escape(symbol)}\s*(?:\([^)=]*\))?\s*=="
    ).search(stripped)
    if declaration is None:
        return None
    body_start = declaration.end()
    next_declaration = re.compile(
        r"(?m)^(?:[A-Za-z_][A-Za-z0-9_]*\s*(?:\([^)=]*\))?\s*==|"
        r"[ \t]*(?:LOCAL[ \t]+)?"
        r"(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)\b|={4,}\s*$)"
    ).search(stripped, body_start)
    body_end = next_declaration.start() if next_declaration is not None else len(stripped)
    return stripped[body_start:body_end], stripped.count("\n", 0, body_start) + 1


def _proofless_release_theorem_errors(
    obligations: list[Any], module_sources: dict[str, str]
) -> list[str]:
    """Require every proofless release theorem to be exact, explicit debt."""

    errors: list[str] = []
    declaration = re.compile(
        r"(?m)^[ \t]*(?:LOCAL[ \t]+)?"
        r"(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)[ \t]+"
        r"([A-Za-z_][A-Za-z0-9_]*)\s*(?:\([^)=\n]*\))?\s*=="
    )
    proof = re.compile(r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b")
    for module, source in module_sources.items():
        if module not in RELEASE_PROOF_MODULES:
            continue
        stripped = strip_tla_comments(source)
        for match in declaration.finditer(stripped):
            symbol = match.group(1)
            extracted = _top_level_theorem_body(source, symbol)
            if extracted is None or proof.search(extracted[0]) is not None:
                continue
            exact_entries = [
                obligation
                for obligation in obligations
                if isinstance(obligation, dict)
                and obligation.get("module") == module
                and obligation.get("symbol") == symbol
            ]
            if len(exact_entries) != 1:
                errors.append(
                    f"{module}.tla:{extracted[1]}: proofless release theorem "
                    f"{module}!{symbol} must have exactly one ledger entry using "
                    "its exact module and symbol"
                )
                continue
            status = exact_entries[0].get("status")
            if status != "specified_unproved":
                errors.append(
                    f"{module}.tla:{extracted[1]}: proofless release theorem "
                    f"{module}!{symbol} must be ledgered specified_unproved, "
                    f"found {status!r}"
                )
    return errors


def _proof_obligation_architecture_errors(
    obligations: list[Any], module_sources: dict[str, str]
) -> list[str]:
    """Bind release obligations to the non-circular production proof layers."""

    errors: list[str] = []
    by_id = {
        obligation.get("id"): obligation
        for obligation in obligations
        if isinstance(obligation, dict) and _nonempty_string(obligation.get("id"))
    }

    for obligation_id, (module, symbol) in FIXED_PROOF_OBLIGATION_TARGETS.items():
        obligation = by_id.get(obligation_id)
        if obligation is None:
            errors.append(f"proof ledger is missing required obligation {obligation_id}")
            continue
        where = f"proof obligation {obligation_id}"
        if obligation.get("module") != module:
            errors.append(f"{where} must use {module}")
        if obligation.get("symbol") != symbol:
            errors.append(f"{where} must use the direct theorem {symbol}")

    def check_direct_theorem(
        obligation_id: str,
        symbol: str,
        *,
        module: str,
        required_spec: str,
        forbidden: tuple[str, ...],
        exact_statement: str | None = None,
    ) -> None:
        obligation = by_id.get(obligation_id)
        if obligation is None:
            errors.append(f"proof ledger is missing required obligation {obligation_id}")
            return
        where = f"proof obligation {obligation_id}"
        if obligation.get("module") != module:
            errors.append(f"{where} must use {module}")
        if obligation.get("symbol") != symbol:
            errors.append(f"{where} must use the direct theorem {symbol}")
        source = module_sources.get(module)
        if source is None:
            return
        declaration = re.compile(
            rf"(?m)^[ \t]*(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)[ \t]+"
            rf"{re.escape(symbol)}\s*=="
        )
        if declaration.search(strip_tla_comments(source)) is None:
            errors.append(
                f"{where} must be one closed theorem universally quantifying initialContext"
            )
        extracted = _top_level_theorem_body(source, symbol)
        if extracted is None:
            return
        body, line = extracted
        statement = re.split(
            r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b", body, maxsplit=1
        )[0]
        normalized_statement = " ".join(statement.split())
        if exact_statement is not None and normalized_statement != exact_statement:
            errors.append(
                f"{module}.tla:{line}: {symbol} must state only "
                f"{exact_statement!r}; found {normalized_statement!r}"
            )
        if re.search(
            rf"\b{re.escape(required_spec)}\s*\(\s*initialContext\s*\)", body
        ) is None:
            errors.append(
                f"{module}.tla:{line}: {symbol} must directly require "
                f"{required_spec}(initialContext)"
            )
        for retired in forbidden:
            if re.search(rf"\b{re.escape(retired)}\b", body):
                errors.append(
                    f"{module}.tla:{line}: {symbol} may not depend on "
                    f"legacy global-barrier operator {retired}"
                )

    def check_closed_theorem(
        obligation_id: str,
        symbol: str,
        *,
        module: str,
        exact_statement: str,
        forbidden: tuple[str, ...],
    ) -> None:
        obligation = by_id.get(obligation_id)
        if obligation is None:
            errors.append(f"proof ledger is missing required obligation {obligation_id}")
            return
        where = f"proof obligation {obligation_id}"
        if obligation.get("module") != module:
            errors.append(f"{where} must use {module}")
        if obligation.get("symbol") != symbol:
            errors.append(f"{where} must use the direct theorem {symbol}")
        source = module_sources.get(module)
        if source is None:
            return
        declaration = re.compile(
            rf"(?m)^[ \t]*(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)[ \t]+"
            rf"{re.escape(symbol)}\s*=="
        )
        if declaration.search(strip_tla_comments(source)) is None:
            errors.append(f"{where} must be one closed, unparameterized theorem")
        extracted = _top_level_theorem_body(source, symbol)
        if extracted is None:
            return
        body, line = extracted
        statement = re.split(
            r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b", body, maxsplit=1
        )[0]
        normalized_statement = " ".join(statement.split())
        if normalized_statement != exact_statement:
            errors.append(
                f"{module}.tla:{line}: {symbol} must state only "
                f"{exact_statement!r}; found {normalized_statement!r}"
            )
        for retired in forbidden:
            if re.search(rf"\b{re.escape(retired)}\b", body):
                errors.append(
                    f"{module}.tla:{line}: {symbol} may not depend on "
                    f"legacy global-barrier operator {retired}"
                )

    for obligation_id, symbol in ARBITRARY_CONTEXT_SAFETY_OBLIGATIONS.items():
        property_wrapper = ARBITRARY_CONTEXT_SAFETY_PROPERTY_WRAPPERS[obligation_id]
        check_direct_theorem(
            obligation_id,
            symbol,
            module="SumeragiV2Proofs",
            required_spec="CoreSpecAt",
            forbidden=("Spec", "NextV2", "AdvanceContext"),
            exact_statement=(
                f"\\A initialContext: "
                f"{property_wrapper}(CoreSpecAt(initialContext))"
            ),
        )
    for obligation_id, symbol in ASYNC_LIVENESS_OBLIGATIONS.items():
        property_wrapper = ASYNC_LIVENESS_PROPERTY_WRAPPERS[obligation_id]
        check_direct_theorem(
            obligation_id,
            symbol,
            module="SumeragiV2AsyncLivenessProofs",
            required_spec="AsyncSpecAt",
            forbidden=("Spec", "NextV2", "AdvanceContext"),
            exact_statement=(
                f"\\A initialContext: "
                f"{property_wrapper}(AsyncSpecAt(initialContext))"
            ),
        )
    for obligation_id, (symbol, property_wrapper) in CHAIN_SAFETY_OBLIGATIONS.items():
        check_closed_theorem(
            obligation_id,
            symbol,
            module="SumeragiV2ChainEpochProofs",
            exact_statement=f"{property_wrapper}(ChainEpochSpec)",
            forbidden=(
                "AsyncSpec",
                "Spec",
                "NextV2",
                "AdvanceContext",
                "CommonAppliedSubject",
            ),
        )
    return errors


def _proof_status_dependency_errors(obligations: list[Any]) -> list[str]:
    """Require compositional theorems to follow and wait for prerequisites."""

    errors: list[str] = []
    by_id: dict[str, dict[str, Any]] = {}
    positions: dict[str, int] = {}
    for index, obligation in enumerate(obligations):
        if not isinstance(obligation, dict):
            continue
        obligation_id = obligation.get("id")
        if not _nonempty_string(obligation_id) or obligation_id in by_id:
            continue
        by_id[obligation_id] = obligation
        positions[obligation_id] = index

    for dependent_id, prerequisite_ids in PROOF_STATUS_DEPENDENCIES.items():
        dependent = by_id.get(dependent_id)
        if dependent is None:
            continue
        for prerequisite_id in prerequisite_ids:
            prerequisite = by_id.get(prerequisite_id)
            if prerequisite is None:
                errors.append(
                    f"proof obligation {dependent_id} is missing prerequisite "
                    f"{prerequisite_id}"
                )
                continue
            if positions[prerequisite_id] >= positions[dependent_id]:
                errors.append(
                    f"proof obligation {dependent_id} must appear after prerequisite "
                    f"{prerequisite_id}"
                )
            if (
                dependent.get("status") == "tlaps_proved"
                and prerequisite.get("status") != "tlaps_proved"
            ):
                errors.append(
                    f"proof obligation {dependent_id} cannot be tlaps_proved before "
                    f"prerequisite {prerequisite_id} is tlaps_proved"
                )
    return errors


def _reachable_oracle_guard_errors(formal_dir: Path) -> list[str]:
    """Reject proof-history oracles from executable Core action bodies."""

    path = formal_dir / "SumeragiV2Core.tla"
    if not path.is_file():
        return []
    source = path.read_text(encoding="utf-8")
    errors: list[str] = []
    for action, forbidden in REACHABLE_ACTION_ORACLES.items():
        extracted = _top_level_operator_body(source, action)
        if extracted is None:
            errors.append(f"{path}: missing oracle-audited Core action {action}")
            continue
        body, first_line = extracted
        for oracle in forbidden:
            for match in re.finditer(rf"\b{re.escape(oracle)}\b", body):
                line = first_line + body.count("\n", 0, match.start())
                errors.append(
                    f"{path}:{line}: executable action {action} may not use "
                    f"proof-history oracle {oracle}"
                )
    return errors


def _async_spec_shape_errors(formal_dir: Path) -> list[str]:
    """Separate arbitrary-context deduction from the finite genesis TLC instance."""

    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    expected = {
        "AsyncBaseInit": "AsyncBaseInitAt(ContextRecord(0, <<>>))",
        "AsyncInitAt": "AsyncBaseInitAt(initialContext) /\\ ViewDomain = Nat",
        "AsyncInit": "AsyncInitAt(ContextRecord(0, <<>>))",
        "AsyncFiniteInitAt": (
            "AsyncBaseInitAt(initialContext) /\\ ViewDomain = FiniteViews"
        ),
        "AsyncFiniteInit": "AsyncFiniteInitAt(ContextRecord(0, <<>>))",
        "AsyncSpec": "AsyncInit /\\ [][AsyncNext]_AsyncAllVars /\\ AsyncFairness",
        "AsyncSpecAt": (
            "AsyncInitAt(initialContext) /\\ [][AsyncNext]_AsyncAllVars "
            "/\\ AsyncFairnessAt(initialContext)"
        ),
        "AsyncFiniteSpec": (
            "AsyncFiniteInit /\\ [][AsyncNext]_AsyncAllVars /\\ AsyncFairness"
        ),
        "AsyncFiniteSpecAt": (
            "AsyncFiniteInitAt(initialContext) /\\ [][AsyncNext]_AsyncAllVars "
            "/\\ AsyncFairnessAt(initialContext)"
        ),
    }
    errors: list[str] = []
    if path.is_file():
        source = path.read_text(encoding="utf-8")
        for symbol, exact_body in expected.items():
            extracted = _top_level_operator_body(source, symbol)
            if extracted is None:
                errors.append(f"{path}: missing required asynchronous operator {symbol}")
                continue
            body, line = extracted
            normalized = " ".join(body.split())
            if normalized != exact_body:
                errors.append(
                    f"{path}:{line}: {symbol} must equal only {exact_body!r}; "
                    f"found {normalized!r}"
                )

    for module in ("SumeragiV2LivenessProofs", "SumeragiV2AsyncLivenessProofs"):
        proof_path = formal_dir / f"{module}.tla"
        if not proof_path.is_file():
            continue
        stripped = strip_tla_comments(proof_path.read_text(encoding="utf-8"))
        for match in re.finditer(r"\bAsyncFiniteSpec\b", stripped):
            line = stripped.count("\n", 0, match.start()) + 1
            errors.append(
                f"{proof_path}:{line}: deductive liveness proofs must use "
                "unbounded AsyncSpec, not the finite TLC instance"
            )
    return errors


def _async_proof_architecture_errors(formal_dir: Path) -> list[str]:
    """Require checked scheduler closure and the exact Core-step refinement."""

    path = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    if not path.is_file():
        return []
    source = path.read_text(encoding="utf-8")
    stripped = strip_tla_comments(source)
    expected = {
        "AsyncStepRefinementObligation": "AsyncNext => [Next]_vars",
        "AsyncTypeInvariantObligation": (
            "\\A initialContext: AsyncSpecAt(initialContext) => []AsyncTypeInvariant"
        ),
        "AsyncNextPreservesNormalProposalPrepareCandidate": (
            "\\A candidate: /\\ NormalProposalPrepareCandidate(candidate) "
            "/\\ AsyncNext => NormalProposalPrepareCandidate(candidate)'"
        ),
    }
    if (
        (formal_dir / "proof_coverage.json").is_file()
        and (formal_dir / "SumeragiV2LivenessProofs.tla").is_file()
    ):
        expected.update({
            "AsyncIngressCapacityGeometry": (
                "ModelConfiguration => "
                "/\\ Cardinality(ValidatorIds) = N "
                "/\\ Cardinality(AsyncIngressSources) = N + 1"
            ),
            "OneRemovalIncreasesSourceProtectionByAtMostOne": (
                "\\A source, before, selected: "
                "/\\ before \\in Seq(Range(before)) "
                "/\\ selected \\in 1..Len(before) => "
                "LET after == SequenceWithoutIndex(before, selected) "
                "IN IngressSourceProtectionPotential(source, after) <= "
                "IngressSourceProtectionPotential(source, before) + 1"
            ),
            "ProtectedProgressSlotUniverseSize": (
                "ModelConfiguration => "
                "/\\ IsFiniteSet(ProtectedProgressSlotUniverse) "
                "/\\ Cardinality(ProtectedProgressSlotUniverse) = 2 * N + 3"
            ),
            "ProtectedProgressSlotIdIsBounded": (
                "\\A command: /\\ N \\in Nat \\ {0} "
                "/\\ AsyncCandidateTyped(command) "
                "/\\ ProtectedProgressCommand(command) "
                "=> ProtectedProgressSlotId(command) \\in 0..(2 * N + 2)"
            ),
            "AppendFreshServeJobPreservesNonceOwnership": (
                "\\A queue, job: /\\ AsyncIoSequenceTyped(queue) "
                "/\\ AsyncIoServeNonceOwnership(queue) "
                "/\\ AsyncIoJobTyped(job) /\\ job.class = \" \" "
                "/\\ job.nonce \\notin {queue[index].nonce: "
                "index \\in AsyncIoServeIndices(queue)} "
                "=> AsyncIoServeNonceOwnership(Append(queue, job))"
            ),
            "AppendNonServeJobPreservesNonceOwnership": (
                "\\A queue, job: /\\ AsyncIoSequenceTyped(queue) "
                "/\\ AsyncIoServeNonceOwnership(queue) "
                "/\\ job.class # \" \" "
                "=> AsyncIoServeNonceOwnership(Append(queue, job))"
            ),
            "TailPreservesServeNonceOwnership": (
                "\\A queue: /\\ AsyncIoSequenceTyped(queue) "
                "/\\ AsyncIoServeNonceOwnership(queue) "
                "/\\ Len(queue) > 0 "
                "=> AsyncIoServeNonceOwnership(Tail(queue))"
            ),
        })
    errors: list[str] = []
    for symbol, exact_statement in expected.items():
        extracted = _top_level_theorem_body(source, symbol)
        if extracted is None:
            errors.append(f"{path}: missing release theorem {symbol}")
            continue
        body, line = extracted
        statement = re.split(
            r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b", body, maxsplit=1
        )[0]
        normalized = " ".join(statement.split())
        if normalized != exact_statement:
            errors.append(
                f"{path}:{line}: {symbol} must state only "
                f"{exact_statement!r}; found {normalized!r}"
            )
    universally_quantified = re.compile(
        r"(?m)^[ \t]*(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)[ \t]+"
        r"AsyncTypeInvariantObligation\s*==\s*\\A\s+initialContext\s*:"
    )
    if universally_quantified.search(stripped) is None:
        errors.append(
            f"{path}: AsyncTypeInvariantObligation must universally quantify initialContext"
        )

    if (formal_dir / "proof_coverage.json").is_file():
        rank_theorems = (
            "ScheduledCandidateServiceRankInCarrier",
            "ProtectedRankExitHasWellFoundedSuccessor",
            "ProtectedRankProgressSuppliesWellFoundedStep",
            "ProtectedServiceRankProgressImpliesStarvation",
        )
        unowned_rank_name = re.compile(
            r"(?<!Owned)ServiceRank(?:Carrier|Ordering)(?![A-Za-z0-9_])"
        )
        for symbol in rank_theorems:
            extracted = _top_level_theorem_body(source, symbol)
            if extracted is None:
                errors.append(f"{path}: missing owned-service-rank theorem {symbol}")
                continue
            body, line = extracted
            if "OwnedServiceRankCarrier" not in body:
                errors.append(
                    f"{path}:{line}: {symbol} must use OwnedServiceRankCarrier"
                )
            if unowned_rank_name.search(body):
                errors.append(
                    f"{path}:{line}: {symbol} may not widen scheduler-owned rank "
                    "proofs to ServiceRankCarrier or ServiceRankOrdering"
                )

        parent_disjoint = _top_level_theorem_body(
            source,
            "CommandSuccessorParentDisjoint",
            preserve_string_contents=True,
        )
        expected_parent_kinds = {
            "AssembleBody",
            "BeginProposal",
            "PersistProposal",
            "DeliverProposal",
            "DeliverChunk",
            "FetchBody",
            "RebindRetainedBody",
            "FetchCertifiedBody",
            "StoreBody",
            "ValidateBody",
            "BeginPrepare",
            "PersistPrepare",
            "DeliverVote",
            "DeliverQC",
            "BeginObservePrepare",
            "PersistObservePrepare",
            "BeginLockCommit",
            "PersistLockCommit",
            "FormCommitQC",
            "BeginDecision",
            "PersistDecision",
            "BeginTimeout",
            "PersistTimeout",
            "DeliverTimeout",
            "FormTC",
            "DeliverTC",
            "BeginInstallTC",
            "PersistInstallTC",
        }
        if parent_disjoint is None:
            errors.append(
                f"{path}: missing exhaustive CommandSuccessorParentDisjoint theorem"
            )
        else:
            body, line = parent_disjoint
            statement = re.split(
                r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b", body, maxsplit=1
            )[0]
            expected_statement = (
                "\\A command: /\\ AsyncCandidateTyped(command) "
                "/\\ command.kind \\in CausalSuccessorParentKinds "
                "=> command \\notin SequenceSet(CommandSuccessors(command))"
            )
            if " ".join(statement.split()) != expected_statement:
                errors.append(
                    f"{path}:{line}: CommandSuccessorParentDisjoint must state "
                    "the exact parent/child ownership-transfer disjointness"
                )
            case_labels = re.findall(
                r'CASE\s+command\.kind\s*=\s*"([A-Za-z0-9]+)"', body
            )
            actual = set(case_labels)
            if actual != expected_parent_kinds or len(case_labels) != len(
                expected_parent_kinds
            ):
                errors.append(
                    f"{path}:{line}: CommandSuccessorParentDisjoint must cover "
                    f"all 28 parents exactly once; missing="
                    f"{sorted(expected_parent_kinds - actual)}, unexpected="
                    f"{sorted(actual - expected_parent_kinds)}"
                )

        explicit_transition_inventory = {
            "Stage4SerializedRuntimeDecreasesAux": (
                "Stage4DeferredDrainDecreasesDebt",
                "Stage4DeferredTagDecreasesDebt",
                "Stage4DirectTimeoutDecreasesDebt",
                "Stage4FifoRuntimeOpensCompletionSlot",
                "Stage4RetransmitDecreasesFifoDebt",
                "Stage4IdleRuntimeMakesReadyActionable",
            ),
            "Stage4BlockedAuxStep": (
                "Stage4StutterPreservesAux",
                "Stage4SameNodeRunDecreasesAux",
                "Stage4OtherRunnerPreservesOrDecreasesAux",
                "Stage4ClockStepPreservesOrDecreasesAux",
                "Stage4DiscoveryPrefixPreservesAux",
                "Stage4IoStepPreservesOrDecreasesAux",
                "Stage4NetworkOrFaultStepPreservesAux",
            ),
            "Stage4CapacitySerializedRuntimeStrictlyProgresses": (
                "Stage4CapacityDeferredDrainStrictlyProgresses",
                "Stage4CapacityDeferredTagStrictlyProgresses",
                "Stage4CapacityDirectTimeoutStrictlyProgresses",
                "Stage4CapacityFifoStrictlyProgresses",
                "Stage4CapacityRetransmitStrictlyProgresses",
                "Stage4CapacityIdleRuntimeIsImpossible",
            ),
            "Stage4CapacityBlockedStep": (
                "Stage4CapacityStutterPreserves",
                "Stage4CapacitySameNodeRunStrictlyProgresses",
                "Stage4CapacityOtherRunnerPreservesOrProgresses",
                "Stage4CapacityClockPreservesOrProgresses",
                "Stage4CapacityDiscoveryPreservesOrProgresses",
                "Stage4CapacityIoPreservesOrProgresses",
                "Stage4CapacityNetworkOrFaultPreservesOrProgresses",
            ),
            "FairStage4CapacityOneStep": (
                "PostGstRunNode",
                "Stage4CapacitySameNodeRunStrictlyProgresses",
                "Stage4CapacityBlockedStep",
                "AsyncFairnessAt",
            ),
            "FairNonCompletionCausalCapacityOpens": (
                "Stage4CapacityRankInCarrier",
                "FairStage4CapacityRankDescent",
            ),
            "Stage4ActionableUnlessProgress": (
                "Stage4ActionableStutterStep",
                "Stage4LocalAdvanceStrictlyProgresses",
                "Stage4ActionableOtherRunnerStep",
                "Stage4ActionableClockStep",
                "Stage4ActionableDiscoveryPrefix",
                "Stage4ActionableIoStep",
                "Stage4ActionableNetworkOrFaultStep",
            ),
            "FairStage4AuxOneStep": (
                "PostGstRunNode",
                "Stage4SameNodeRunDecreasesAux",
                "Stage4BlockedAuxStep",
                "AsyncFairnessAt",
            ),
            "FairProtectedStage4RankDescent": (
                "FairStage4AuxRankDescent",
                "FairNonCompletionCausalCapacityOpens",
                "FairStage4ActionableProgress",
            ),
            "FairProtectedStage5RankDescent": (
                "PostGstServiceIoWorker",
                "ProtectedStage5WorkerStrictlyProgresses",
                "ProtectedStage5UnlessProgress",
                "AsyncFairnessAt",
            ),
            "FairCommitCertificateDiscoveryPublishesOrDecides": (
                "CommitCertificateDiscoveryPendingEnablesFairPrefix",
                "DirectCommitCertificateDiscoveryPublishes",
                "CommitCertificateDiscoveryPendingUnlessOutcome",
                "PostGstCommitCertificateDiscovery",
                "WF_AsyncAllVars",
                "AsyncFairnessAt",
            ),
            "CommitCertificateDiscoveryPendingUnlessOutcome": (
                "AsyncBracketNextPreservesStrongTypeInvariant",
                "GstAsyncStepIsMonotone",
                "AsyncBracketNextPreservesDiscoveryClockThreshold",
                "CommitCertificateRequestOutboxNonemptyIffRemoteVoter",
                "CommitCertificateDiscoveryOutcome",
            ),
            "AsyncNonRunnerStepPreservesDiscoveryClockThreshold": (
                "AsyncSetGST",
                "AsyncTick",
                "DirectCommitCertificateDiscoveryStep",
                "ServiceIoWorker",
                "EnqueueIoLocalControl",
                "AsyncNetworkStep",
                "AsyncFaultStepLeavesDiscoveryClock",
            ),
            "AsyncBracketNextPreservesDiscoveryClockThreshold": (
                "AsyncRunnerStepLeavesDiscoveryClock",
                "AsyncNonRunnerStepPreservesDiscoveryClockThreshold",
                "PreGstCrash",
                "UNCHANGED AsyncAllVars",
            ),
            "OneHeightCompletionObligation": (
                "RotatingLeaderProgressObligation",
                "ApplicationLivenessObligation",
            ),
        }
        forbidden_transition_automation = re.compile(
            r"(?i)\b(?:SMT|SMTT|AXIOM|OBVIOUS|OMITTED)\b"
        )
        for symbol, required in explicit_transition_inventory.items():
            extracted = _top_level_theorem_body(source, symbol)
            if extracted is None:
                errors.append(f"{path}: missing explicit rank theorem {symbol}")
                continue
            body, line = extracted
            missing = [token for token in required if token not in body]
            if missing:
                errors.append(
                    f"{path}:{line}: {symbol} omits explicit transition/fairness "
                    f"inventory {missing}"
                )
            if forbidden_transition_automation.search(body):
                errors.append(
                    f"{path}:{line}: {symbol} may not hide transition induction "
                    "behind SMT/axiom/obvious/omitted discharge"
                )

    vocabulary_path = formal_dir / "SumeragiV2LivenessProofs.tla"
    if vocabulary_path.is_file():
        vocabulary_source = vocabulary_path.read_text(encoding="utf-8")
        property_contracts = {
            "ResponsiveNodesDecide": (
                r"\A node \in AsyncCurrentResponsiveVoters: NodeHasDecision(node)"
            ),
            "ResponsiveNodesApply": (
                r"\A node \in AsyncCurrentResponsiveVoters: NodeHasApplication(node)"
            ),
            "ResponsiveHonestLeaderViewReached": (
                r"\E leader \in (AsyncCurrentResponsiveVoters \cap Honest): "
                r"/\ ~NodeHasDecision(leader) "
                r"/\ Leader(context, nodeView[leader]) = leader"
            ),
            "TimeoutViewProgressProperty": (
                r"specification => \A node \in AsyncCurrentResponsiveVoters, "
                r"roundView \in Views: (gst /\ nodeView[node] = roundView /\ "
                r"~NodeHasDecision(node)) ~> (nodeView[node] > roundView \/ "
                r"NodeHasDecision(node))"
            ),
            "RotatingLeaderProgressProperty": (
                r"specification => /\ (gst /\ ~ResponsiveNodesDecide) "
                r"~> (ResponsiveHonestLeaderViewReached \/ "
                r"ResponsiveNodesDecide) /\ (gst /\ "
                r"ResponsiveHonestLeaderViewReached /\ "
                r"~ResponsiveNodesDecide) ~> ResponsiveNodesDecide"
            ),
            "ApplicationLivenessProperty": (
                r"specification => /\ \A node \in "
                r"AsyncCurrentResponsiveVoters: (gst /\ "
                r"NodeHasDecision(node)) ~> NodeHasApplication(node) "
                r"/\ (gst /\ ResponsiveNodesDecide) ~> ResponsiveNodesApply"
            ),
        }
        string_property_contracts: dict[str, str] = {}
        if (formal_dir / "proof_coverage.json").is_file():
            property_contracts.update({
              "GenerationScopedVoteDeliveryProperty": (
                r"specification => [][VoteDeliveryEpochAction]_AsyncAllVars"
              ),
              "ProgressWitnessProperty": (
                r"specification => []ProgressWitnessInvariant"
              ),
              "DeadlockFreedomProperty": (
                r"specification => [](gst /\ ~ResponsiveNodesDecide "
                r"=> PostGstProductiveActionEnabled)"
              ),
              "HeightProtocolEvidenceGrows": (
                r"\/ SetGains(availableBodies, availableBodies') "
                r"\/ SetGains(durableBodies, durableBodies') "
                r"\/ SetGains(retainedLockedBodies, retainedLockedBodies') "
                r"\/ SetGains(validatedBodies, validatedBodies') "
                r"\/ SetGains(seenProposals, seenProposals') "
                r"\/ SetGains(receivedVotes, receivedVotes') "
                r"\/ SetGains(receivedQCs, receivedQCs') "
                r"\/ SetGains(proposalIntents, proposalIntents') "
                r"\/ SetGains(prepareIntents, prepareIntents') "
                r"\/ SetGains(commitIntents, commitIntents') "
                r"\/ SetGains(prepareQCs, prepareQCs') "
                r"\/ SetGains(commitQCs, commitQCs') "
                r"\/ SetGains(decisions, decisions') "
                r"\/ SetGains(applied, applied')"
              ),
              "SetGains": r"after \ before # {}",
              "DeadlineDistance": (
                r"IF now < deadline THEN deadline - now ELSE 0"
              ),
              "PostGstProductiveStep": (
                r"/\ gst /\ AsyncNext /\ \/ HeightProtocolEvidenceGrows "
                r"\/ PostGstDeadlineDebtDecreases "
                r"\/ ProtectedServiceRankDecreaseStep "
                r"\/ ProtectedServeRankDecreaseStep"
              ),
              "PostGstProductiveActionEnabled": (
                r"ENABLED PostGstProductiveStep"
              ),
              "PostGstDeadlineDebtDecreases": (
                r"\/ \E node \in AsyncCurrentResponsiveVoters: "
                r"\/ DeadlineDistance(asyncNodeDeadlines'[node], asyncNow') "
                r"< DeadlineDistance(asyncNodeDeadlines[node], asyncNow) "
                r"\/ DeadlineDistance(asyncRetransmitDeadlines'[node], "
                r"asyncNow') < DeadlineDistance("
                r"asyncRetransmitDeadlines[node], asyncNow) "
                r"\/ DeadlineDistance(asyncNodeServiceDeadlines'[node], "
                r"asyncNow') < DeadlineDistance("
                r"asyncNodeServiceDeadlines[node], asyncNow) "
                r"\/ DeadlineDistance(asyncIoServiceDeadlines'[node], "
                r"asyncNow') < DeadlineDistance("
                r"asyncIoServiceDeadlines[node], asyncNow) "
                r"\/ \E packet \in asyncTransport \cap asyncTransport': "
                r"DeadlineDistance(packet.deadline, asyncNow') < "
                r"DeadlineDistance(packet.deadline, asyncNow)"
              ),
              "ProtectedServiceRankDecreaseStep": (
                r"\E candidate \in AsyncCandidateSet, stage \in 2..6, "
                r"position \in Nat: "
                r"/\ ResponsiveProtectedCandidateOwned(candidate) "
                r"/\ CandidateServiceRank(candidate) = <<stage, position>> "
                r"/\ \/ ~ResponsiveProtectedCandidateOwned(candidate)' "
                r"\/ ServiceRankLess(CandidateServiceRank(candidate)', "
                r"<<stage, position>>)"
              ),
              "ResponsiveProtectedCandidateOwned": (
                r"/\ candidate.node \in AsyncCurrentResponsiveVoters "
                r"/\ ProtectedCandidateOwned(candidate)"
              ),
              "ProtectedServiceRankProgressProperty": (
                r"specification => \A candidate \in AsyncCandidateSet, "
                r"stage \in 2..6, position \in Nat: (gst /\ "
                r"ResponsiveProtectedCandidateOwned(candidate) /\ "
                r"CandidateServiceRank(candidate) = <<stage, position>>) "
                r"~> (~ResponsiveProtectedCandidateOwned(candidate) \/ "
                r"ServiceRankLess(CandidateServiceRank(candidate), "
                r"<<stage, position>>))"
              ),
              "ResponsiveProtectedServeJobOwned": (
                r"/\ node \in AsyncCurrentResponsiveVoters "
                r"/\ job \in AsyncServeJobSet "
                r"/\ job \in SequenceSet(asyncIoQueues[node])"
              ),
              "ServeJobIndex": (
                r"CHOOSE index \in "
                r"AsyncIoServeIndices(asyncIoQueues[node]): "
                r"asyncIoQueues[node][index] = job"
              ),
              "ServeJobRank": r"<<5, ServeJobIndex(node, job)>>",
              "ProtectedServeRankDecreaseStep": (
                r"\E node \in AsyncCurrentResponsiveVoters, "
                r"job \in AsyncServeJobSet, position \in Nat: "
                r"/\ ResponsiveProtectedServeJobOwned(node, job) "
                r"/\ ServeJobRank(node, job) = <<5, position>> "
                r"/\ \/ ~ResponsiveProtectedServeJobOwned(node, job)' "
                r"\/ ServiceRankLess(ServeJobRank(node, job)', "
                r"<<5, position>>)"
              ),
              "ProtectedServeRankProgressProperty": (
                r"specification => \A node \in "
                r"AsyncCurrentResponsiveVoters, job \in AsyncServeJobSet, "
                r"position \in Nat: (gst /\ "
                r"ResponsiveProtectedServeJobOwned(node, job) /\ "
                r"ServeJobRank(node, job) = <<5, position>>) ~> "
                r"(~ResponsiveProtectedServeJobOwned(node, job) \/ "
                r"ServiceRankLess( ServeJobRank(node, job), "
                r"<<5, position>>))"
              ),
              "ProtectedServeStarvationProperty": (
                r"specification => \A node \in "
                r"AsyncCurrentResponsiveVoters, job \in AsyncServeJobSet: "
                r"(gst /\ ResponsiveProtectedServeJobOwned(node, job)) "
                r"~> ~ResponsiveProtectedServeJobOwned(node, job)"
              ),
              "NormalProposalPrepareRankProgressProperty": (
                r"specification => \A candidate \in AsyncCandidateSet, "
                r"stage \in 2..6, position \in Nat: (gst /\ "
                r"ResponsiveProtectedCandidateOwned(candidate) /\ "
                r"NormalProposalPrepareCandidate(candidate) /\ "
                r"CandidateServiceRank(candidate) = <<stage, position>>) "
                r"~> (~ResponsiveProtectedCandidateOwned(candidate) \/ "
                r"ServiceRankLess(CandidateServiceRank(candidate), "
                r"<<stage, position>>))"
              ),
              "StarvationFreedomProperty": (
                r"/\ (specification => \A candidate \in "
                r"AsyncCandidateSet: (gst /\ "
                r"ResponsiveProtectedCandidateOwned(candidate)) ~> "
                r"~ResponsiveProtectedCandidateOwned(candidate)) "
                r"/\ ProtectedServeStarvationProperty(specification)"
              ),
            })
        theorem_contracts: dict[str, str] = {}
        if (formal_dir / "proof_coverage.json").is_file():
            theorem_contracts.update({
                "RuntimeReachRankIsNatural": (
                    r"AsyncTypeInvariant => \A node \in ValidatorIds: "
                    r"RuntimeReachRank(node) \in Nat"
                ),
                "RetransmissionBudgetCoversEveryClass": (
                    r"ModelConfiguration /\ AsyncConfiguration => "
                    r"/\ AsyncRetainedControlBudget \in Nat "
                    r"/\ AsyncRetainedProposalChunkBudget \in Nat "
                    r"/\ AsyncActiveCertifiedRequestBudget \in Nat "
                    r"/\ AsyncActiveCommitRequestBudget \in Nat "
                    r"/\ AsyncActiveRequestBudget = "
                    r"AsyncActiveCertifiedRequestBudget + "
                    r"AsyncActiveCommitRequestBudget "
                    r"/\ AsyncRetransmitEmissionBudget = "
                    r"AsyncRetainedControlBudget + "
                    r"AsyncRetainedProposalChunkBudget + AsyncActiveRequestBudget"
                ),
                "CanonicalSuccessorPreservesAdmissibility": (
                    r"ModelConfiguration => \A initialContext \in ContextRecords, "
                    r"subject \in ValidSubjects: "
                    r"(FrozenContextAdmissible(initialContext) /\ "
                    r"initialContext.height < MaxHeight) => "
                    r"FrozenContextAdmissible( "
                    r"CanonicalSuccessorContext(initialContext, subject))"
                ),
            })
            string_property_contracts.update({
              "AsyncServeJobSet": (
                r'{AsyncIoJob("Serve", candidate, nonce): '
                r"candidate \in AsyncCandidateSet, "
                r"nonce \in 0..AsyncIoAuxCapacity}"
              ),
              "NormalProposalPrepareNoItemKinds": (
                r'{"AssembleBody", "BeginPrepare"}'
              ),
              "NormalProposalPrepareNetworkKinds": (
                r'{"Proposal", "PrepareVote", "CommitVote"}'
              ),
              "NormalBeginPrepareParentKinds": (
                r'{"DeliverProposal", "ValidateBody"}'
              ),
              "FrozenNormalDeliveryCandidate": (
                r"LET subject == DeliverySubject(item) IN "
                r'AsyncCandidateWithIdentity( "Normal", DeliveryKind(item), '
                r"item.envelope.recipient, DeliveryHeight(item), "
                r"DeliveryView(item), subject, item, consumerContext, "
                r"consumerView, consumerGeneration, item, subject, subject, "
                r"subject)"
              ),
              "NormalDeliveryCandidate": (
                r"FrozenNormalDeliveryCandidate( item, context, "
                r"nodeView[item.envelope.recipient], "
                r"generation[item.envelope.recipient])"
              ),
              "FrozenNormalAssemblyCandidate": (
                r'AsyncCandidateWithIdentity( "Normal", "AssembleBody", '
                r"node, blockContext.height, roundView, subject, NoAsyncItem, "
                r"blockContext, roundView, consumerGeneration, evidence, "
                r"subject, subject, subject)"
              ),
              "NextCandidateGeneration": (
                r"IF currentGeneration < MaxGeneration THEN "
                r"currentGeneration + 1 ELSE currentGeneration"
              ),
              "FrozenInstallProposalSuccessor": (
                r'AsyncCandidateWithIdentity( "Normal", "AssembleBody", '
                r"command.node, installedContext.height, command.view + 1, "
                r"subject, NoAsyncItem, installedContext, command.view + 1, "
                r"NextCandidateGeneration(priorGeneration), command.evidence, "
                r"subject, subject, subject)"
              ),
              "FrozenNormalBeginPrepareCandidate": (
                r'AsyncCandidateWithIdentity( "Normal", "BeginPrepare", '
                r"parent.node, blockHeight, parent.view, parent.subject, "
                r"NoAsyncItem, parent.consumerContext, parent.consumerView, "
                r"parent.consumerGeneration, parent.evidence, "
                r"parent.bodyIdentity, parent.manifestIdentity, "
                r"parent.commitmentIdentity)"
              ),
              "NormalProposalPrepareNoItemCandidate": (
                r'/\ candidate.item = NoAsyncItem '
                r'/\ candidate.kind \in NormalProposalPrepareNoItemKinds '
                r'/\ \/ \E blockContext \in ContextRecords, '
                r'node \in ValidatorIds, roundView \in Views, '
                r'consumerGeneration \in Generations, '
                r'subject \in SubjectOrNone: candidate = '
                r'FrozenNormalAssemblyCandidate( blockContext, node, '
                r'roundView, consumerGeneration, subject, NoAsyncItem) '
                r'\/ \E command \in AsyncCandidateSet, '
                r'installedContext \in ContextRecords, '
                r'priorGeneration \in Generations, '
                r'subject \in SubjectOrNone: '
                r'/\ command.kind = "PersistInstallTC" '
                r'/\ command.view + 1 \in Views '
                r'/\ candidate = FrozenInstallProposalSuccessor( command, '
                r'installedContext, priorGeneration, subject) '
                r'\/ \E parent \in AsyncCandidateSet, '
                r'blockHeight \in Heights: '
                r'/\ parent.kind \in NormalBeginPrepareParentKinds '
                r'/\ candidate = '
                r'FrozenNormalBeginPrepareCandidate(parent, blockHeight)'
              ),
              "NormalProposalPrepareNetworkCandidate": (
                r'\E item \in AsyncNetworkItems, '
                r'consumerContext \in ContextRecords, '
                r'consumerView \in Views, '
                r'consumerGeneration \in Generations: '
                r'/\ item.kind \in NormalProposalPrepareNetworkKinds '
                r'/\ candidate = FrozenNormalDeliveryCandidate( item, '
                r'consumerContext, consumerView, consumerGeneration)'
              ),
              "NormalProposalPrepareCandidate": (
                r'/\ candidate \in AsyncCandidateSet '
                r'/\ candidate.class = "Normal" '
                r'/\ \/ NormalProposalPrepareNoItemCandidate(candidate) '
                r'\/ NormalProposalPrepareNetworkCandidate(candidate)'
              ),
              "ProtectedServiceCandidate": (
                r'/\ candidate \in AsyncCandidateSet '
                r'/\ \/ candidate.class = "Completion" '
                r'\/ /\ candidate.class = "Progress" '
                r'/\ candidate.kind # "RejectProgress" '
                r'\/ NormalProposalPrepareCandidate(candidate)'
              ),
            })
        for symbol, exact_body in property_contracts.items():
            extracted = _top_level_operator_body(vocabulary_source, symbol)
            if extracted is None:
                errors.append(
                    f"{vocabulary_path}: missing stable liveness property {symbol}"
                )
                continue
            body, line = extracted
            normalized = " ".join(body.split())
            if normalized != exact_body:
                errors.append(
                    f"{vocabulary_path}:{line}: {symbol} must equal only "
                    f"{exact_body!r}; found {normalized!r}"
                )
        for symbol, exact_body in string_property_contracts.items():
            extracted = _top_level_operator_body(
                vocabulary_source, symbol, preserve_string_contents=True
            )
            if extracted is None:
                errors.append(
                    f"{vocabulary_path}: missing stable liveness property {symbol}"
                )
                continue
            body, line = extracted
            normalized = " ".join(body.split())
            if normalized != exact_body:
                errors.append(
                    f"{vocabulary_path}:{line}: {symbol} must equal only "
                    f"{exact_body!r}; found {normalized!r}"
                )
        for symbol, exact_statement in theorem_contracts.items():
            extracted = _top_level_theorem_body(vocabulary_source, symbol)
            if extracted is None:
                errors.append(
                    f"{vocabulary_path}: missing release theorem {symbol}"
                )
                continue
            body, line = extracted
            statement = re.split(
                r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b", body, maxsplit=1
            )[0]
            normalized = " ".join(statement.split())
            if normalized != exact_statement:
                errors.append(
                    f"{vocabulary_path}:{line}: {symbol} must state only "
                    f"{exact_statement!r}; found {normalized!r}"
                )

        if (formal_dir / "proof_coverage.json").is_file():
            candidate_rank = _top_level_operator_body(
                vocabulary_source, "CandidateServiceRank"
            )
            if candidate_rank is None:
                errors.append(
                    f"{vocabulary_path}: missing scheduler-owned "
                    "CandidateServiceRank"
                )
            else:
                rank_body, rank_line = candidate_rank
                forbidden_rank_tokens = (
                    "CandidateInIngress",
                    "CandidateInTransport",
                    "<<7,",
                    "<<8,",
                )
                present = [
                    token for token in forbidden_rank_tokens if token in rank_body
                ]
                if present:
                    errors.append(
                        f"{vocabulary_path}:{rank_line}: CandidateServiceRank must "
                        "be scheduler-owned stages 2..6; transport and ingress "
                        f"require occurrence-specific proofs, found {present}"
                    )

        safety_path = formal_dir / "SumeragiV2Proofs.tla"
        if safety_path.is_file():
            safety_source = strip_tla_comments(safety_path.read_text(encoding="utf-8"))
            safety_liveness_symbols = set(property_contracts) | {
                "NodeHasDecision",
                "NodeHasApplication",
                "DecisionBodyReady",
                "HeightLivenessProperty",
            }
            for symbol in safety_liveness_symbols:
                if _symbol_exists(safety_source, symbol):
                    errors.append(
                        f"{safety_path}: asynchronous liveness symbol {symbol} "
                        "may not be redeclared in the safety proof module"
                    )
    return errors


def _generalized_context_init_errors(formal_dir: Path) -> list[str]:
    """Require the deductive Core entry point to cover every admissible height."""

    path = formal_dir / "SumeragiV2Core.tla"
    if not path.is_file():
        return []
    source = path.read_text(encoding="utf-8")
    errors: list[str] = []
    init = _top_level_operator_body(source, "Init")
    if init is None:
        errors.append(f"{path}: missing required Core operator Init")
    else:
        body, line = init
        normalized = " ".join(body.split())
        expected = "InitAt(ContextRecord(0, <<>>))"
        if normalized != expected:
            errors.append(
                f"{path}:{line}: Init must be the finite genesis wrapper {expected!r}; "
                f"found {normalized!r}"
            )

    init_at = _top_level_operator_body(source, "InitAt")
    if init_at is None:
        errors.append(f"{path}: missing arbitrary-context Core operator InitAt")
    else:
        body, line = init_at
        if not re.search(r"\bFrozenContextAdmissible\s*\(\s*initialContext\s*\)", body):
            errors.append(
                f"{path}:{line}: InitAt must require "
                "FrozenContextAdmissible(initialContext)"
            )

    admissible = _top_level_operator_body(source, "FrozenContextAdmissible")
    if admissible is None:
        errors.append(f"{path}: missing FrozenContextAdmissible")
    else:
        body, line = admissible
        required = ("ContextRecords", "initialContext.lineage", "ValidSubjects")
        missing = [token for token in required if token not in body]
        if missing:
            errors.append(
                f"{path}:{line}: FrozenContextAdmissible must bind syntactic context, "
                f"lineage, and external validity; missing {missing}"
            )
    return errors


def _safety_property_source_fidelity_errors(formal_dir: Path) -> list[str]:
    """Pin the exact arbitrary-context safety claims exposed to the ledger."""

    path = formal_dir / "SumeragiV2Proofs.tla"
    if not path.is_file():
        return []
    source = path.read_text(encoding="utf-8")
    property_contracts = {
        "DurableVoteUniquenessProperty": (
            "specification => [](/\\ HonestPrepareUniqueness "
            "/\\ HonestCommitUniqueness /\\ HonestTimeoutUniqueness)"
        ),
        "LockMonotonicityProperty": (
            "specification => [][LockMonotonicityAction]_vars"
        ),
        "ExternalValidityProperty": (
            "specification => [](/\\ \\A qc \\in prepareQCs: "
            "qc.subject \\in ValidSubjects /\\ \\A qc \\in commitQCs: "
            "qc.subject \\in ValidSubjects /\\ \\A decision \\in decisions: "
            "decision.qc.subject \\in ValidSubjects)"
        ),
        "CertifiedBodyAvailabilityProperty": (
            "specification => [](/\\ PrepareCertificateAvailability "
            "/\\ CommitCertificateAvailability)"
        ),
        "CertificateUniquenessProperty": (
            "specification => []CertificateUniquenessInvariant"
        ),
        "PotentialCommitVotes": (
            '{vote \\in commitIntents: /\\ vote.context = certificateContext '
            '/\\ vote.view = roundView /\\ vote.phase = "Commit" '
            "/\\ vote.subject = subject}"
        ),
        "PotentialCommitSigners": (
            "{vote.signer: vote \\in PotentialCommitVotes( "
            "certificateContext, roundView, subject)}"
        ),
        "InstalledTcAuthorizedPotentialCommitIntersection": (
            "\\E timeoutVote \\in tc.votes, commitVote \\in "
            "PotentialCommitVotes( tc.context, protectedView, subject): "
            "/\\ timeoutVote.signer \\in Honest "
            "/\\ commitVote.signer = timeoutVote.signer "
            "/\\ timeoutVote.context = tc.context "
            "/\\ timeoutVote.view = tc.view "
            "/\\ ~TimeoutVoteStrictlyProtectsCommit(timeoutVote, commitVote) "
            "/\\ InstalledTcAuthorizesCommitVote(commitVote)"
        ),
        "TCProtectsOrInstalledTcAuthorizesPotentialCommit": (
            "\\A protectedView \\in 0..tc.view, subject \\in Subjects: "
            "DualQuorum(tc.context.epoch, PotentialCommitSigners(tc.context, "
            "protectedView, subject)) => "
            "\\/ TCProtectsViewSubject(tc, protectedView, subject) "
            "\\/ InstalledTcAuthorizedPotentialCommitIntersection( tc, "
            "protectedView, subject)"
        ),
        "TimeoutProtectionProperty": (
            "specification => [](\\A tc \\in formedTCs: "
            "TCProtectsOrInstalledTcAuthorizesPotentialCommit(tc))"
        ),
        "AgreementProperty": "specification => []DecisionAgreement",
        "NoConflictingCommitCertificatesProperty": (
            "specification => [](\\A left, right \\in commitQCs: "
            "left.context = right.context => left.subject = right.subject)"
        ),
        "CrashRecoveryProperty": (
            "/\\ (specification => []CrashRecoveryStateInvariant) "
            "/\\ (specification => [][CrashPreservesDurableProjection]_vars) "
            "/\\ (specification => [][RestartPreservesDurableProjection]_vars) "
            "/\\ (specification => [][PendingWritesAreUnacknowledged]_vars) "
            "/\\ (specification => "
            "[][TypeInvariant => StaleGenerationRejected]_vars)"
        ),
    }
    errors: list[str] = []
    for symbol, exact_body in property_contracts.items():
        extracted = _top_level_operator_body(
            source, symbol, preserve_string_contents=True
        )
        if extracted is None:
            errors.append(f"{path}: missing stable safety property {symbol}")
            continue
        body, line = extracted
        normalized = " ".join(body.split())
        if normalized != exact_body:
            errors.append(
                f"{path}:{line}: {symbol} must equal only "
                f"{exact_body!r}; found {normalized!r}"
            )
    return errors


def _historical_timeout_derivation_errors(formal_dir: Path) -> list[str]:
    """Keep historical Commit authorization derived, not duplicated."""

    invariant_path = formal_dir / "SumeragiV2Inductive.tla"
    proof_path = formal_dir / "SumeragiV2InductiveProofs.tla"
    if not invariant_path.is_file() or not proof_path.is_file():
        return []

    errors: list[str] = []
    invariant_source = invariant_path.read_text(encoding="utf-8")
    for symbol in (
        "ReducerProvenanceInvariant",
        "ReducerProvenanceWithoutVoteTransport",
        "ReducerProvenanceWithoutTimeoutTransport",
    ):
        extracted = _top_level_operator_body(invariant_source, symbol)
        if extracted is None:
            errors.append(f"{invariant_path}: missing reducer provenance {symbol}")
            continue
        body, line = extracted
        if "HistoricalTcLockedCommitAuthorizationInvariant" in body:
            errors.append(
                f"{invariant_path}:{line}: {symbol} may not duplicate the derived "
                "historical timeout/Commit authorization invariant"
            )

    proof_source = proof_path.read_text(encoding="utf-8")
    symbol = "ReducerProvenanceImpliesHistoricalTcLockedCommitAuthorization"
    extracted = _top_level_theorem_body(proof_source, symbol)
    if extracted is None:
        errors.append(f"{proof_path}: missing derived historical authorization theorem")
        return errors
    body, line = extracted
    statement = re.split(
        r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b", body, maxsplit=1
    )[0]
    normalized = " ".join(statement.split())
    expected = (
        "ReducerProvenanceInvariant => "
        "HistoricalTcLockedCommitAuthorizationInvariant"
    )
    if normalized != expected:
        errors.append(
            f"{proof_path}:{line}: {symbol} must state only {expected!r}; "
            f"found {normalized!r}"
        )
    return errors


def _progress_witness_source_fidelity_errors(formal_dir: Path) -> list[str]:
    """Require decision recovery owners to match the decided block height."""

    if not (formal_dir / "proof_coverage.json").is_file():
        return []
    path = formal_dir / "SumeragiV2LivenessProofs.tla"
    if not path.is_file():
        return []
    source = path.read_text(encoding="utf-8")
    required = {
        "DecisionPipelineCandidate": "candidate.height = qc.context.height",
        "DecisionCompletionWitness": (
            "request.envelope.height = qc.context.height"
        ),
    }
    errors: list[str] = []
    for symbol, required_equality in required.items():
        extracted = _top_level_operator_body(source, symbol)
        if extracted is None:
            errors.append(f"{path}: missing source-fidelity operator {symbol}")
            continue
        body, line = extracted
        normalized = " ".join(body.split())
        if required_equality not in normalized:
            errors.append(
                f"{path}:{line}: {symbol} must require exact decision-height "
                f"ownership via {required_equality!r}"
            )
    return errors


def _async_source_fidelity_errors(formal_dir: Path) -> list[str]:
    """Reject async-model shortcuts that previously made progress circular."""

    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    if not path.is_file():
        return []
    source = path.read_text(encoding="utf-8")
    stripped = strip_tla_comments(source)
    errors: list[str] = []

    for symbol in (
        "asyncCertifiedHeight",
        "decidedAt",
        "nodeHeight",
        "nodeContext",
    ):
        for match in re.finditer(rf"\b{re.escape(symbol)}\b", stripped):
            line = stripped.count("\n", 0, match.start()) + 1
            errors.append(
                f"{path}:{line}: proof-only shadow chain state {symbol} is prohibited"
            )

    exact = {
        "AsyncChunkReceiptSet": (
            "[node: ValidatorIds, view: Views, subject: Subjects, "
            "chunk: AsyncChunks]"
        ),
        "AsyncBodyEnvelopeSet": (
            "[recipient: ValidatorIds, height: Heights, view: Views, "
            "subject: Subjects, chunk: 0..AsyncChunkCount, "
            "nonce: 0..(AsyncIngressCapacity - 1)]"
        ),
        "AsyncSetGST": (
            '/\\ ~gst '
            '/\\ asyncRecoveryPhase \\notin '
            '{"RestartRequired", "ReplayRequired", "Replaying"} '
            "/\\ Responsive \\subseteq up /\\ SetGST "
            "/\\ UNCHANGED <<AsyncSchedulerVars, AsyncRecoveryVars>>"
        ),
        "AsyncRecoveryVars": (
            "<<asyncRecoveryPhase, asyncRecoveryNode, asyncRecoveryGeneration, "
            "asyncRecoveryReplayQueue>>"
        ),
        "AsyncRecoveryLifecycleVars": (
            "<<asyncRecoveryPhase, asyncRecoveryNode, asyncRecoveryGeneration>>"
        ),
        "AsyncRecoveryPhases": (
            '{"Eligible", "RestartRequired", "ReplayRequired", "Replaying", '
            '"Recovered"}'
        ),
        "ResponsiveReplayQuarantined": (
            '/\\ node = asyncRecoveryNode /\\ asyncRecoveryPhase \\in '
            '{"ReplayRequired", "Replaying"}'
        ),
        "ResponsiveReplayDraining": (
            'node = asyncRecoveryNode /\\ asyncRecoveryPhase = "Replaying"'
        ),
        "ResponsiveReplayExecutorAllowed": (
            "~ResponsiveReplayQuarantined(node) \\/ "
            "ResponsiveReplayDraining(node)"
        ),
        "ResponsiveReplayServiceIoWorker": (
            "LET node == asyncRecoveryNode IN /\\ ~gst "
            "/\\ ResponsiveReplayDraining(node) "
            "/\\ ServiceIoWorker(node) "
            "/\\ UNCHANGED <<up, AsyncRecoveryVars>>"
        ),
        "ReplayCommitIntentReady": (
            "\\/ VoteSign(node, vote) \\in signVotes "
            "\\/ \\E item \\in asyncRetainedControl: "
            '/\\ item.kind = "CommitVote" '
            "/\\ item.source = node /\\ item.envelope.vote = vote "
            "\\/ VoteAt(node, vote) \\in receivedVotes "
            "\\/ \\E qc \\in commitQCs: "
            "/\\ qc.context = vote.context /\\ qc.view = vote.view "
            "/\\ qc.subject = vote.subject \\/ NodeHasDecision(node)"
        ),
        "ReplayCommitSourcesReady": (
            "\\A vote \\in RestartLockedCommitIntents(node): "
            "ReplayCommitIntentReady(node, vote)"
        ),
        "RestartTimeoutOrProposalReplay": (
            "IF RestartTimeoutIntents(node) # {} "
            "THEN RestartTimeoutReplay(node) "
            "ELSE IF RestartProposalIntents(node) # {} "
            "THEN RestartProposalReplay(node) ELSE <<>>"
        ),
        "RestartPrepareReplayIfActive": (
            "IF RestartPrepareIntents(node) # {} "
            "THEN RestartPrepareReplay(node) ELSE <<>>"
        ),
        "RestartLockedCommitReplayIfActive": (
            "IF RestartLockedCommitIntents(node) # {} "
            "THEN RestartLockedCommitReplay(node) ELSE <<>>"
        ),
        "RestartSignatureReplay": (
            "IF NodeHasApplication(node) \\/ RestartDecisions(node) # {} "
            "THEN <<>> ELSE RestartTimeoutOrProposalReplay(node) "
            "\\o RestartPrepareReplayIfActive(node) "
            "\\o RestartLockedCommitReplayIfActive(node)"
        ),
        "RestartReplay": (
            "IF NodeHasApplication(node) THEN <<>> "
            "ELSE IF RestartDecisions(node) # {} "
            "THEN RestartDecisionReplay(node) "
            "ELSE LET signatures == RestartSignatureReplay(node) "
            "IN IF Len(signatures) > 0 THEN <<Head(signatures)>> "
            "ELSE RestartRunnerAssembly(node)"
        ),
        "AsyncRestartAuthorityInvariant": (
            "asyncRecoveryPhase \\in "
            '{"RestartRequired", "ReplayRequired", "Replaying"} '
            "=> generation[asyncRecoveryNode] = asyncRecoveryGeneration"
        ),
        "AsyncAllVars": "<<vars, AsyncSchedulerVars, AsyncRecoveryVars>>",
        "RetainedControlEmissionItems": (
            "SendableItems(node) \\cup RetainedProposalChunks(node)"
        ),
        "AsyncBaseInit": "AsyncBaseInitAt(ContextRecord(0, <<>>))",
        "AsyncStepRefinesCore": "AsyncNext => [Next]_vars",
        "CertifiedServeCanRespond": (
            '/\\ request.kind = "CertifiedRequest" '
            "/\\ BodyHeldBy(durableBodies, request.envelope.recipient, "
            "context, request.envelope.view, request.envelope.subject)"
        ),
        "NextCommandClass": (
            'CASE commandClass = "Completion" -> "Progress" '
            '[] commandClass = "Progress" -> "Normal" '
            '[] OTHER -> "Completion"'
        ),
        "SelectedDeferredClass": (
            "LET first == asyncNextDeferredClass[node] "
            "second == NextCommandClass(first) "
            "third == NextCommandClass(second) "
            "IN IF DeferredClassNonempty(node, first) THEN first "
            "ELSE IF DeferredClassNonempty(node, second) THEN second ELSE third"
        ),
        "NextDeferredCommand": (
            "Head(DeferredClassQueue(node, SelectedDeferredClass(node)))"
        ),
        "AdvanceNextDeferredClass": (
            "asyncNextDeferredClass' = [asyncNextDeferredClass EXCEPT "
            "![node] = NextCommandClass(SelectedDeferredClass(node))]"
        ),
        "RemoveNextDeferredCommand": (
            '/\\ IF SelectedDeferredClass(node) = "Completion" '
            "THEN /\\ asyncDeferredCompletionQueues' = "
            "[asyncDeferredCompletionQueues EXCEPT ![node] = Tail(@)] "
            "/\\ UNCHANGED <<asyncDeferredProgressQueues, "
            "asyncDeferredNormalQueues>> "
            'ELSE IF SelectedDeferredClass(node) = "Progress" '
            "THEN /\\ asyncDeferredProgressQueues' = "
            "[asyncDeferredProgressQueues EXCEPT ![node] = Tail(@)] "
            "/\\ UNCHANGED <<asyncDeferredCompletionQueues, "
            "asyncDeferredNormalQueues>> "
            "ELSE /\\ asyncDeferredNormalQueues' = "
            "[asyncDeferredNormalQueues EXCEPT ![node] = Tail(@)] "
            "/\\ UNCHANGED <<asyncDeferredCompletionQueues, "
            "asyncDeferredProgressQueues>> "
            "/\\ AdvanceNextDeferredClass(node)"
        ),
        "SelectedCommandClass": (
            "LET first == asyncNextCommandClass[node] "
            "second == NextCommandClass(first) "
            "third == NextCommandClass(second) "
            "IN IF CommandClassIndices(node, first) # {} "
            "THEN first ELSE IF CommandClassIndices(node, second) # {} "
            "THEN second ELSE third"
        ),
        "NextNodeCommandIndex": (
            "FirstCommandClassIndex(node, SelectedCommandClass(node))"
        ),
        "RemoveNextNodeCommand": (
            "/\\ asyncCommandQueues' = [asyncCommandQueues EXCEPT "
            "![node] = SequenceWithoutIndex(@, NextNodeCommandIndex(node))] "
            "/\\ asyncNextCommandClass' = [asyncNextCommandClass EXCEPT "
            "![node] = NextCommandClass(SelectedCommandClass(node))]"
        ),
        "SchedulerClassPrefixIndices": (
            "{index \\in 1..Len(asyncCommandQueues[node]): "
            "/\\ asyncCommandQueues[node][index].class = command.class "
            "/\\ \\E matching \\in SchedulerCandidateIndices(node, command): "
            "index <= matching}"
        ),
        "SchedulerServiceRank": (
            "3 * Cardinality(SchedulerClassPrefixIndices(node, command)) "
            "+ CommandClassDistance(asyncNextCommandClass[node], command.class)"
        ),
        "CommandExecutionEnabled": (
            "\\E selectedCommand \\in {command}: "
            "\\/ ENABLED ExecuteRegularCommand(selectedCommand) "
            "\\/ ENABLED ExecuteDecisionFetch(selectedCommand) "
            "\\/ ENABLED ExecuteSignProposal(selectedCommand) "
            "\\/ ENABLED ExecuteSignVote(selectedCommand) "
            "\\/ ENABLED ExecuteFormPrepareQC(selectedCommand) "
            "\\/ ENABLED ExecuteSignTimeout(selectedCommand) "
            "\\/ ENABLED ExecutePersistInstall(selectedCommand) "
            "\\/ ENABLED ExecutePersistDecision(selectedCommand) "
            "\\/ ENABLED ExecuteRequestCertifiedBody(selectedCommand) "
            "\\/ ENABLED ExecuteApply(selectedCommand) "
            "\\/ ENABLED ExecuteCoreDelivery(selectedCommand) "
            "\\/ ENABLED ExecuteChunkDelivery(selectedCommand) "
            "\\/ ENABLED ExecuteRejectAuthenticatedJunk(selectedCommand)"
        ),
        "CommandDispatchable": (
            "/\\ AsyncCandidateTyped(command) "
            "/\\ CandidateConsumerCurrent(command) "
            "/\\ CommandExecutionEnabled(command) "
            "/\\ (NodeIdle(command.node) "
            "\\/ command.class = \"Completion\")"
        ),
        "FreshCandidateSequence": (
            "IF CandidateScheduled(candidate) THEN <<>> ELSE <<candidate>>"
        ),
        "CausalSuccessorParentKinds": (
            '{"AssembleBody", "BeginProposal", "PersistProposal", '
            '"DeliverProposal", "DeliverChunk", "FetchBody", '
            '"RebindRetainedBody", "FetchCertifiedBody", "StoreBody", '
            '"ValidateBody", "BeginPrepare", "PersistPrepare", "DeliverVote", '
            '"DeliverQC", "BeginObservePrepare", "PersistObservePrepare", '
            '"BeginLockCommit", "PersistLockCommit", "FormCommitQC", '
            '"BeginDecision", "PersistDecision", "BeginTimeout", '
            '"PersistTimeout", "DeliverTimeout", "FormTC", "DeliverTC", '
            '"BeginInstallTC", "PersistInstallTC"}'
        ),
        "FreshCommandSuccessors": (
            "LET successors == CommandSuccessors(command) "
            "IN CASE Len(successors) = 0 -> <<>> "
            "[] Len(successors) = 1 -> FreshCandidateSequence(successors[1]) "
            "[] Len(successors) = 2 -> "
            "FreshCandidateSequence(successors[1]) "
            "\\o FreshCandidateSequence(successors[2]) "
            "[] Len(successors) = 3 -> "
            "FreshCandidateSequence(successors[1]) "
            "\\o FreshCandidateSequence(successors[2]) "
            "\\o FreshCandidateSequence(successors[3]) "
            "[] OTHER -> <<>>"
        ),
        "AppendCausalSuccessors": (
            "asyncCausalQueues' = [asyncCausalQueues EXCEPT "
            "![command.node] = @ \\o FreshCommandSuccessors(command)]"
        ),
    }
    if (formal_dir / "proof_coverage.json").is_file():
        exact.update({
            "IngressProgressKinds": (
                '{"CommitVote", "PrepareQC", "CommitQC", "TimeoutVote", '
                '"TimeoutCertificate", "Chunk", "CertifiedRequest", '
                '"CertifiedResponse", "CommitCertificateRequest", '
                '"CommitCertificateResponse"}'
            ),
            "IngressLaneHasNonTimeoutProgressIn": (
                "\\E queued \\in SequenceSet(lanes[recipient][source]): "
                '/\\ IngressAdmissionClass(queued) = "Progress" '
                '/\\ queued.kind # "TimeoutVote"'
            ),
            "IngressProtectedSourcesFor": (
                "{source \\in AsyncIngressSources: "
                "\\/ Len(lanes[recipient][source]) = 0 "
                "\\/ /\\ source \\in ValidatorIds "
                "/\\ ~IngressLaneHasNonTimeoutProgressIn( "
                "lanes, recipient, source)}"
            ),
            "IngressTimeoutVoteProtectedSourcesFor": (
                "{source \\in ValidatorIds: "
                "~IngressLaneHasTimeoutVoteIn(lanes, recipient, source)}"
            ),
            "IngressContinuationProtectedSourcesFor": (
                "{source \\in ValidatorIds: "
                "\\/ Len(lanes[recipient][source]) = 0 "
                "\\/ /\\ Len(lanes[recipient][source]) = 1 "
                "/\\ (IngressLaneHasNonTimeoutProgressIn( "
                "lanes, recipient, source) "
                "\\/ IngressLaneHasTimeoutVoteIn( "
                "lanes, recipient, source)) "
                "\\/ /\\ Len(lanes[recipient][source]) = 2 "
                "/\\ IngressLaneHasNonTimeoutProgressIn( "
                "lanes, recipient, source) "
                "/\\ IngressLaneHasTimeoutVoteIn( "
                "lanes, recipient, source)}"
            ),
            "IngressProtectedSlotCountFor": (
                "Cardinality(IngressProtectedSourcesFor(lanes, recipient)) + "
                "Cardinality( IngressTimeoutVoteProtectedSourcesFor("
                "lanes, recipient)) + "
                "Cardinality(IngressContinuationProtectedSourcesFor(lanes, recipient))"
            ),
            "IngressProtectedSlotCountAfterAdmission": (
                "IngressProtectedSlotCountFor(IngressLanesAfterAdmission(item), "
                "item.envelope.recipient)"
            ),
            "AsyncIngressCapacityTypeInvariant": (
                "\\A recipient \\in ValidatorIds: "
                "/\\ \\A source \\in AsyncIngressSources: "
                "IngressLaneDepth(recipient, source) <= AsyncIngressCapacity "
                "/\\ IngressDepth(recipient) <= AsyncIngressCapacity "
                "/\\ IngressDepth(recipient) + "
                "IngressProtectedSlotCountFor( asyncIngressLanes, recipient) "
                "<= AsyncIngressCapacity"
            ),
            "IngressUsableCapacityAfterAdmission": (
                "AsyncIngressCapacity - IngressProtectedSlotCountAfterAdmission(item)"
            ),
            "AsyncValidTimeoutVoteWireByteBound": "4 * 1024",
            "AsyncTimeoutVoteByteReserve": "64 * 1024",
            "AsyncTimeoutVoteByteGateAllows": (
                '\\/ item.kind # "TimeoutVote" '
                "\\/ item.source \\notin ValidatorIds "
                "\\/ /\\ AsyncValidTimeoutVoteWireByteBound <= "
                "AsyncTimeoutVoteByteReserve "
                "/\\ ~IngressLaneHasTimeoutVoteIn(asyncIngressLanes, "
                "item.envelope.recipient, item.source)"
            ),
            "IngressLaneHasTimeoutVoteIn": (
                "\\E queued \\in SequenceSet(lanes[recipient][source]): "
                'queued.kind = "TimeoutVote"'
            ),
            "ProtectedProgressCommand": (
                'CASE command.kind = "DeliverVote" -> '
                "HistoricalLockedCommitItem(command.item) "
                '[] command.kind = "DeliverTimeout" -> '
                'command.item.kind = "TimeoutVote" '
                '[] command.kind = "DeliverQC" -> '
                'command.item.kind \\in {"PrepareQC", "CommitQC"} '
                '[] command.kind = "DeliverTC" -> '
                'command.item.kind = "TimeoutCertificate" '
                "[] OTHER -> FALSE"
            ),
            "SameProtectedProgressSlot": (
                "/\\ ProtectedProgressCommand(left) "
                "/\\ ProtectedProgressCommand(right) "
                "/\\ left.node = right.node "
                '/\\ CASE left.kind = "DeliverVote" -> '
                '/\\ right.kind = "DeliverVote" '
                "/\\ left.item.envelope.vote.signer = "
                "right.item.envelope.vote.signer "
                '[] left.kind = "DeliverQC" -> '
                '/\\ right.kind = "DeliverQC" '
                "/\\ left.item.kind = right.item.kind "
                '[] left.kind = "DeliverTimeout" -> '
                '/\\ right.kind = "DeliverTimeout" '
                "/\\ left.item.envelope.vote.signer = "
                "right.item.envelope.vote.signer "
                '[] OTHER -> right.kind = "DeliverTC"'
            ),
            "DeferredProgressAfter": (
                "LET queue == asyncDeferredProgressQueues[node] "
                "IN IF command \\in SequenceSet(queue) THEN queue "
                "ELSE IF SameProtectedProgressSlotIndices(node, command) # {} "
                "THEN queue ELSE IF Len(queue) < "
                "AsyncDeferredProgressCapacity THEN Append(queue, command) "
                "ELSE queue"
            ),
            "DeliveryClass": (
                "IF HistoricalLockedCommitItem(item) "
                '\\/ item.kind \\in {"PrepareQC", "CommitQC", '
                '"TimeoutVote", "TimeoutCertificate", "Chunk", '
                '"CertifiedResponse", "CommitCertificateResponse", '
                '"ProgressJunk"} THEN "Progress" ELSE "Normal"'
            ),
            "AsyncIoServeIndices": (
                '{index \\in 1..Len(queue): queue[index].class = "Serve"}'
            ),
            "AsyncIoServeNonces": (
                "{asyncIoQueues[node][index].nonce: "
                "index \\in AsyncIoServeIndices(asyncIoQueues[node])}"
            ),
            "FreshAsyncIoServeNonce": (
                "CHOOSE nonce \\in 0..AsyncIoAuxCapacity: "
                "nonce \\notin AsyncIoServeNonces(node)"
            ),
            "AsyncIoCertifiedServeJob": (
                'AsyncIoJob("Serve", candidate, FreshAsyncIoServeNonce(node))'
            ),
            "AsyncIoServeNonceOwnership": (
                "\\A left, right \\in AsyncIoServeIndices(queue): "
                "queue[left].nonce = queue[right].nonce => left = right"
            ),
            "AsyncIoQueueContentTypeInvariant": (
                "\\A node \\in ValidatorIds: "
                "/\\ AsyncIoSequenceTyped(asyncIoQueues[node]) "
                "/\\ AsyncIoServeNonceOwnership(asyncIoQueues[node]) "
                "/\\ \\A job \\in SequenceSet(asyncIoQueues[node]): "
                'job.class = "Consensus" => '
                "job.candidate \\in asyncOutstandingWork[node] "
                "/\\ AsyncIoConsensusCandidateOwnership( node, "
                "asyncIoQueues, asyncIoReadyCompletions, "
                "asyncLocalReadyCompletions)"
            ),
            "CanAdmitIngressItem": (
                "/\\ IngressDepth(item.envelope.recipient) < "
                "IngressUsableCapacityAfterAdmission(item) "
                "/\\ AsyncTimeoutVoteByteGateAllows(item)"
            ),
        })
    for symbol, expected in exact.items():
        extracted = _top_level_operator_body(
            source, symbol, preserve_string_contents=True
        )
        if extracted is None:
            errors.append(f"{path}: missing source-fidelity operator {symbol}")
            continue
        body, line = extracted
        normalized = " ".join(body.split())
        if normalized != expected:
            errors.append(
                f"{path}:{line}: {symbol} must equal only {expected!r}; "
                f"found {normalized!r}"
            )

    for retired in (
        "DominatedProtectedProgressIndices",
        "ReplaceableUnprotectedProgressIndices",
        "FirstProgressIndex",
    ):
        if re.search(rf"(?m)^{retired}\s*(?:\([^\n]*\))?\s*==", stripped):
            errors.append(
                f"{path}: retired displacement operator {retired} is prohibited"
            )

    expected_successor_parents = {
        "AssembleBody",
        "BeginProposal",
        "PersistProposal",
        "DeliverProposal",
        "DeliverChunk",
        "FetchBody",
        "RebindRetainedBody",
        "FetchCertifiedBody",
        "StoreBody",
        "ValidateBody",
        "BeginPrepare",
        "PersistPrepare",
        "DeliverVote",
        "DeliverQC",
        "BeginObservePrepare",
        "PersistObservePrepare",
        "BeginLockCommit",
        "PersistLockCommit",
        "FormCommitQC",
        "BeginDecision",
        "PersistDecision",
        "BeginTimeout",
        "PersistTimeout",
        "DeliverTimeout",
        "FormTC",
        "DeliverTC",
        "BeginInstallTC",
        "PersistInstallTC",
    }
    successor_relation = _top_level_operator_body(
        source, "CommandSuccessors", preserve_string_contents=True
    )
    if successor_relation is None:
        errors.append(f"{path}: missing source-fidelity operator CommandSuccessors")
    else:
        successor_body, successor_line = successor_relation
        case_labels = re.findall(
            r'command\.kind\s*=\s*"([A-Za-z0-9]+)"\s*->', successor_body
        )
        actual_successor_parents = set(case_labels)
        if (
            actual_successor_parents != expected_successor_parents
            or len(case_labels) != len(expected_successor_parents)
        ):
            missing = sorted(expected_successor_parents - actual_successor_parents)
            unexpected = sorted(actual_successor_parents - expected_successor_parents)
            errors.append(
                f"{path}:{successor_line}: CommandSuccessors parent inventory "
                "must be closed under scheduler-wide coalescing; "
                f"missing={missing}, unexpected={unexpected}, "
                f"duplicate_labels={len(case_labels) - len(actual_successor_parents)}"
            )
        fetch_branch = re.search(
            r'\[\] command\.kind = "FetchBody"\s*->(?P<body>.*?)'
            r'(?=\n\s*\[\] command\.kind = "RebindRetainedBody")',
            successor_body,
            re.DOTALL,
        )
        expected_fetch_branch = (
            "IF DecisionFetchFrontier(command) "
            "THEN IF BodyHeldBy(durableBodies, command.node, context, "
            "command.view, command.subject) "
            'THEN <<CausalCandidate("Completion", "ValidateBody", command)>> '
            "ELSE <<>> "
            'ELSE <<CausalCandidate("Completion", "StoreBody", command)>>'
        )
        if fetch_branch is None:
            errors.append(
                f"{path}:{successor_line}: CommandSuccessors is missing the "
                "Decision FetchBody frontier"
            )
        else:
            fetch_normalized = " ".join(fetch_branch.group("body").split())
            if fetch_normalized != expected_fetch_branch:
                errors.append(
                    f"{path}:{successor_line}: FetchBody successors must equal "
                    "only the durable-body Validate frontier, certified-request "
                    "wait, or ordinary StoreBody frontier; found "
                    f"{fetch_normalized!r}"
                )
        persist_decision_branch = re.search(
            r'\[\] command\.kind = "PersistDecision"\s*->(?P<body>.*?)'
            r'(?=\n\s*\[\] command\.kind = "BeginTimeout")',
            successor_body,
            re.DOTALL,
        )
        expected_persist_decision_branch = (
            '<<CausalCandidate("Completion", "FetchBody", command)>>'
        )
        if persist_decision_branch is None:
            errors.append(
                f"{path}:{successor_line}: CommandSuccessors is missing the "
                "durable Decision FetchBody frontier"
            )
        else:
            persist_decision_normalized = " ".join(
                persist_decision_branch.group("body").split()
            )
            if persist_decision_normalized != expected_persist_decision_branch:
                errors.append(
                    f"{path}:{successor_line}: PersistDecision must schedule "
                    "exactly one FetchBody frontier; found "
                    f"{persist_decision_normalized!r}"
                )

    liveness_path = formal_dir / "SumeragiV2LivenessProofs.tla"
    if liveness_path.is_file():
        liveness_source = liveness_path.read_text(encoding="utf-8")
        extracted = _top_level_operator_body(
            liveness_source,
            "DeferredCandidatePosition",
            preserve_string_contents=True,
        )
        expected = (
            "3 * Cardinality( DeferredClassPrefixIndices(candidate.node, "
            "candidate)) + CommandClassDistance( "
            "asyncNextDeferredClass[candidate.node], candidate.class)"
        )
        if extracted is None:
            errors.append(
                f"{liveness_path}: missing source-fidelity operator "
                "DeferredCandidatePosition"
            )
        else:
            body, line = extracted
            normalized = " ".join(body.split())
            if normalized != expected:
                errors.append(
                    f"{liveness_path}:{line}: DeferredCandidatePosition must "
                    f"equal only {expected!r}; found {normalized!r}"
                )

    async_liveness_path = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    if async_liveness_path.is_file():
        async_liveness_source = async_liveness_path.read_text(encoding="utf-8")
        restart_theorems = {
            "RestartDecisionOwnsOneFetchFrontier": (
                '\\A node: RestartDecisions(node) # {} => '
                "/\\ Len(RestartDecisionReplay(node)) = 1 "
                '/\\ RestartDecisionReplay(node)[1].kind = "FetchBody"'
            ),
            "RestartSignatureReplayExactOrder": (
                "\\A node: RestartSignatureReplay(node) = "
                "IF NodeHasApplication(node) \\/ RestartDecisions(node) # {} "
                "THEN <<>> ELSE RestartTimeoutOrProposalReplay(node) "
                "\\o RestartPrepareReplayIfActive(node) "
                "\\o RestartLockedCommitReplayIfActive(node)"
            ),
            "AppliedRecoveryCannotScheduleSameHeightAssembly": (
                "\\A node: NodeHasApplication(node) => "
                "RestartRunnerAssembly(node) = <<>>"
            ),
            "AppliedRecoverySchedulesNoSameHeightWork": (
                "\\A node: NodeHasApplication(node) => "
                "RestartReplay(node) = <<>>"
            ),
            "RestartSignatureReplayProperties": (
                "\\A node \\in ValidatorIds: TypeInvariant => "
                "/\\ AsyncQueueTyped(RestartSignatureReplay(node)) "
                "/\\ AsyncCausalQueueOwnership(node, "
                "RestartSignatureReplay(node)) "
                "/\\ SequenceHasUniqueValues(RestartSignatureReplay(node)) "
                "/\\ Len(RestartSignatureReplay(node)) <= 3"
            ),
            "RestartReplayIsTypedOwnedAndUnique": (
                "\\A node \\in ValidatorIds: StrongInductiveInvariant => "
                "/\\ AsyncQueueTyped(RestartReplay(node)) "
                "/\\ AsyncCausalQueueOwnership(node, RestartReplay(node)) "
                "/\\ SequenceHasUniqueValues(RestartReplay(node)) "
                "/\\ Len(RestartReplay(node)) <= 1"
            ),
        }
        for symbol, expected_statement in restart_theorems.items():
            extracted = _top_level_theorem_body(
                async_liveness_source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                errors.append(
                    f"{async_liveness_path}: missing restart source-fidelity "
                    f"theorem {symbol}"
                )
                continue
            theorem_body, theorem_line = extracted
            statement = re.split(
                r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b",
                theorem_body,
                maxsplit=1,
            )[0]
            normalized_statement = " ".join(statement.split())
            if normalized_statement != expected_statement:
                errors.append(
                    f"{async_liveness_path}:{theorem_line}: {symbol} must "
                    f"state only {expected_statement!r}; found "
                    f"{normalized_statement!r}"
                )

    required_body_tokens = {
        "ServiceIoWorker": (
            "asyncIoControlAvailable'",
            "EXCEPT ![node] = TRUE",
            "ResponsiveReplayExecutorAllowed(node)",
            "CommitCertificateServeCanRespond",
            "CommitCertificateResponseItems",
        ),
        "CertifiedRequestOutbox": ("qc.signers \\ {node}",),
        "SendNodeRetransmissions": ("RetryableItems(node)",),
        "AsyncTickEnabled": ("~gst", "OverdueResponsivePackets"),
        "AsyncRunnerStep": ("RunNode(node)", "RunHistoricalServer(node)"),
        "AsyncNonRunnerStep": (
            "AsyncSetGST",
            "AsyncTick",
            "DirectCommitCertificateDiscoveryStep(node)",
            "ServiceIoWorker(node)",
            "EnqueueIoLocalControl(node)",
            "AsyncNetworkStep",
            "AsyncFaultStep",
        ),
        "AsyncNext": ("AsyncNonCrashStep", "PreGstCrash(node)"),
        "AsyncSchedulerVars": (
            "asyncNextCommandClass",
            "asyncNextDeferredClass",
        ),
        "AsyncRuntimeInit": (
            "asyncNextCommandClass =",
            '[node \\in ValidatorIds |-> "Completion"]',
        ),
        "AsyncRuntimeScalarTypeInvariant": (
            "asyncNextCommandClass \\in [ValidatorIds -> AsyncCommandClasses]",
        ),
        "AsyncDeferredInit": (
            "asyncNextDeferredClass =",
            '[node \\in ValidatorIds |-> "Completion"]',
        ),
        "AsyncDeferredTopologyTypeInvariant": (
            "asyncNextDeferredClass \\in",
            "[ValidatorIds -> AsyncCommandClasses]",
        ),
        "DeferredDrainStep": (
            "NextDeferredCommand(node)",
            "RemoveNextDeferredCommand(node)",
            "AdvanceNextDeferredClass(node)",
            "UNCHANGED <<vars, asyncCommandQueues,",
            "asyncDeferredCompletionQueues,",
            "asyncDeferredProgressQueues,",
            "asyncDeferredNormalQueues,",
        ),
        "AsyncBodyEnvelopeTyped": ("envelope.subject \\in Subjects",),
        "FifoRuntimeStep": (
            "NextNodeCommand(node)",
            "RemoveNextNodeCommand(node)",
        ),
        "RegularCoreCommand": (
            'command.kind = "ValidateBody"',
            "ValidateBody(command.node, proposal)",
            "DecisionQcValues",
            "ValidateDecidedBody(command.node, qc)",
        ),
        "RunNode": (
            "~NodeHasApplication(node)",
            "IF ResponsiveReplayQuarantined(node)",
            "ResponsiveReplayDraining(node)",
            "asyncIngressReady[node] = <<>>",
            "LocalAdmissionStep(node)",
            "IngressDrainStep(node)",
            "SerializedRuntimeStep(node)",
        ),
        "RunHistoricalServer": (
            "~ResponsiveReplayQuarantined(node)",
            "NodeHasApplication(node)",
            "DrainHistoricalIngressSelected(node)",
        ),
        "ResponsiveReplayRunNode": (
            "node == asyncRecoveryNode",
            "~gst",
            "ResponsiveReplayDraining(node)",
            "RunNode(node)",
            "UNCHANGED <<up, AsyncRecoveryVars>>",
        ),
        "HistoricalIngressItemCanDrain": (
            'item.kind = "CertifiedRequest"',
            'item.kind = "CommitCertificateRequest"',
        ),
        "HistoricalDrainableIngressLaneIndices": (
            "HistoricalIngressItemCanDrain(",
            "IngressLane(node, source)[index]",
        ),
        "HistoricalIngressSourceCanDrain": (
            "HistoricalDrainableIngressLaneIndices(node, source)",
            "# {}",
        ),
        "HistoricalSelectedIngressLaneIndex": (
            "FirstHistoricalDrainableIngressLaneIndex(",
            "asyncIngressReady[node][index]",
        ),
        "HistoricalSelectedIngressItemAt": (
            "HistoricalSelectedIngressLaneIndex(node, index)",
        ),
        "ItemInScheduledDelivery": (
            "QueuedCandidates",
            "DeferredCandidates",
            "CausalCandidates",
            "TrackedWorkCandidates",
            "candidate.item = item",
            "CandidateConsumerCurrent(candidate)",
        ),
        "IngressItemCanDrain": (
            "CandidateScheduled(candidate)",
            "CanEnqueueClass(node, candidate.class)",
            "~CompletionCausalAdmissionDebt(node)",
            "~NonCompletionCausalAdmissionDebt(node)",
        ),
        "CausalAdmissionDebtActive": (
            "asyncCausalAdmissionOwed[node]",
            "CausalQueueNonempty(node)",
        ),
        "NonCompletionCausalAdmissionDebt": (
            "CausalAdmissionDebtActive(node)",
            'HeadCausalCandidate(node).class # "Completion"',
        ),
        "CompletionCausalAdmissionDebt": (
            "CausalAdmissionDebtActive(node)",
            'HeadCausalCandidate(node).class = "Completion"',
        ),
        "ProducerCompletionCanAdvance": (
            "ProducerCompletionCanAdmit(node)",
            "~NonCompletionCausalAdmissionDebt(node)",
        ),
        "LocalSourceCanAdmit": ("ProducerCompletionCanAdvance(node)",),
        "LocalAdmissionCanAdvance": ("ProducerCompletionCanAdvance(node)",),
        "RecordBlockedCausalDebt": (
            "asyncCausalAdmissionOwed' =",
            "CausalQueueNonempty(node)",
            "UNCHANGED asyncNextLocalSource",
        ),
        "AdmitProducerCompletion": ("ProducerCompletionCanAdvance(node)",),
        "EnqueueIoLocalControl": ("~CompletionCausalAdmissionDebt(node)",),
        "LocalAdmissionStep": ("RecordBlockedCausalDebt(node)",),
        "DrainableIngressLaneIndices": (
            "IngressItemCanDrain(node, IngressLane(node, source)[index])",
        ),
        "IngressSourceCanDrain": (
            "DrainableIngressLaneIndices(node, source)",
            "# {}",
        ),
        "SelectedIngressLaneIndex": (
            "FirstDrainableIngressLaneIndex(",
            "asyncIngressReady[node][index]",
        ),
        "SelectedIngressItemAt": ("SelectedIngressLaneIndex(node, index)",),
        "PopSelectedIngress": (
            "SequenceWithoutIndex(@, laneIndex)",
            "ReadyAfterSelectedDrain(node, index)",
        ),
        "DirectCommitCertificateDiscoveryStep": (
            "CommitCertificateDiscoveryDue(node)",
            "UNCHANGED <<vars, asyncNow,",
            "asyncCommandQueues, asyncNextCommandClass,",
            "asyncFifoOwed, asyncTimeoutEmitted,",
            "asyncRunnerPhase, asyncRunnerBudget,",
            "AsyncLocalAdmissionVars, AsyncIoVars,",
            "AsyncDeferredVars,",
            "asyncNodeServiceDeadlines, asyncIoServiceDeadlines,",
            "PublishCommitCertificateRequests(",
            "CommitCertificateRequestOutbox(node)",
        ),
        "PostGstCommitCertificateDiscovery": (
            "gst",
            "DirectCommitCertificateDiscoveryStep(node)",
        ),
        "CommitCertificateDiscoveryDue": (
            "asyncNow >= AsyncRoundTimeout",
            "~NodeHasDecision(node)",
            "ActiveCommitCertificateRequests(node) = {}",
            "CommitCertificateRequestOutbox(node) # {}",
        ),
        "CommitCertificateResponseAuthorized": (
            'item.kind = "CommitCertificateResponse"',
            "MatchingCommitCertificateRequests(item)",
        ),
        "DrainFairIngressSelected": (
            "SelectedIngressLaneIndex(node, index)",
            "SelectedIngressItemAt(node, index)",
            "PopSelectedIngress(node, index, laneIndex)",
            "CommitCertificateResponseAuthorized(item)",
            "CommitCertificateResponseCandidate(item)",
            "EnqueueCandidate(discoveredCandidate)",
            "MatchingCommitCertificateRequests(item)",
            "CandidateScheduled(candidate)",
            "AsyncIoCertifiedServeJob(",
            "node, candidate",
        ),
        "DrainHistoricalIngressSelected": (
            "HistoricalSelectedIngressLaneIndex(node, index)",
            "HistoricalSelectedIngressItemAt(node, index)",
            "PopSelectedIngress(node, index, laneIndex)",
            "AsyncIoCertifiedServeJob(",
            "node, candidate",
        ),
        "AsyncFairnessAt": (
            "WF_AsyncAllVars(AsyncSetGST)",
            "WF_AsyncAllVars(PreGstResponsiveRestart)",
            "WF_AsyncAllVars(PreGstResponsiveReplay)",
            "WF_AsyncAllVars(ResponsiveReplayRunNode)",
            "WF_AsyncAllVars(ResponsiveReplayServiceIoWorker)",
            "WF_AsyncAllVars(DriveResponsiveReplayHead)",
            "WF_AsyncAllVars(FinishResponsiveReplay)",
            "PostGstRunNode(node)",
            "PostGstRunHistoricalServer(node)",
            "PostGstCommitCertificateDiscovery(node)",
            "PostGstServiceIoWorker(node)",
            "PostGstAdmitHiddenPacket(recipient, source)",
        ),
        "VoteOutbox": (
            "recipient \\in CurrentVoters \\ {request.node}",
        ),
        "InstallCommandSuccessors": (
            "InstallCommitSignRequests(command)",
            "InstallCommitSignSuccessor(command)",
            "InstallProposalSuccessor(command)",
        ),
        "CommandSuccessors": (
            'command.kind = "FetchBody"',
            "DecisionFetchFrontier(command)",
            "BodyHeldBy(durableBodies, command.node, context,",
            'CausalCandidate("Completion", "ValidateBody", command)',
            'CausalCandidate("Completion", "StoreBody", command)',
        ),
        "AsyncProgressOwnershipInvariant": (
            "AsyncLogicalCandidateOwnershipInvariant",
            "AsyncOutstandingCarrierInvariant",
            "SerializedBusyOwnershipInvariant",
            "BusyCompletionWitnessInvariant",
        ),
        "BusyCompletionWitnessInvariant": (
            "~NodeIdle(node)",
            "BusyCompletionCandidates(node)",
            "ActiveBusyCompletionCarrier",
        ),
        "ActiveBusyCompletionCarrier": (
            "QueuedCandidates",
            "CausalCandidates",
            "TrackedWorkCandidates",
        ),
        "ExecuteDecisionFetch": (
            "DecisionFetchFrontier(command)",
            "BodyHeldBy(durableBodies, command.node, context, command.view, command.subject)",
            "THEN /\\ UNCHANGED vars /\\ UNCHANGED <<asyncSentItems, asyncRetainedControl, asyncActiveRequests, asyncTransport>>",
            "decision.node = command.node",
            "decision.qc.context = context",
            "decision.qc.view = command.view",
            "decision.qc.subject = command.subject",
            'decision.qc.phase = "Commit"',
            "PublishCertifiedRequests(",
            "CertifiedRequestOutbox(command.node, decision.qc)",
        ),
        "ExecuteCommand": (
            "ExecuteRegularCommand(command)",
            "ExecuteDecisionFetch(command)",
        ),
        "RestartRunnerAssemblyEnabled": (
            "node \\in Honest \\cap up \\cap CurrentVoters",
            "node = Leader(context, nodeView[node])",
            "~NodeHasApplication(node)",
            "RestartDecisions(node) = {}",
            "~NodeTimedOut(node, nodeView[node])",
            "~BodyHeldBy(durableBodies, node, context, nodeView[node],",
        ),
        "RestartDecisions": (
            "decision \\in decisions",
            "decision.node = node",
            "decision.qc.context = context",
            'decision.qc.phase = "Commit"',
            "[node |-> node, qc |-> decision.qc] \\notin applied",
        ),
        "RestartLockedCommitIntents": (
            "vote \\in commitIntents",
            "vote.context = context",
            "vote.signer = node",
            'vote.phase = "Commit"',
            "vote.view = lockRank[node]",
            "vote.subject = lockSubject[node]",
        ),
        "RestartTimeoutIntents": (
            "vote \\in timeoutIntents",
            "vote.context = context",
            "vote.signer = node",
            "vote.view = nodeView[node]",
        ),
        "RestartPrepareIntents": (
            "vote \\in prepareIntents",
            "vote.context = context",
            "vote.signer = node",
            'vote.phase = "Prepare"',
            "vote.view = nodeView[node]",
            "RestartTimeoutIntents(node) = {}",
        ),
        "RestartProposalIntents": (
            "proposal \\in proposalIntents",
            "proposal.context = context",
            "proposal.proposer = node",
            "proposal.view = nodeView[node]",
            "RestartTimeoutIntents(node) = {}",
        ),
        "RestartDecisionReplay": (
            'RestartCandidate("Completion", "FetchBody", node,',
            "qc.view, qc.subject, qc)",
        ),
        "RestartLockedCommitReplay": (
            'RestartCandidate("Completion", "SignVote", node,',
            "vote.view, vote.subject, vote)",
        ),
        "RestartTimeoutReplay": (
            'RestartCandidate("Completion", "SignTimeout", node,',
            "vote.view, vote.highSubject, vote)",
        ),
        "RestartPrepareReplay": (
            'RestartCandidate("Completion", "SignVote", node,',
            "vote.view, vote.subject, vote)",
        ),
        "RestartProposalReplay": (
            'RestartCandidate("Completion", "SignProposal", node,',
            "proposal.view, proposal.subject, proposal)",
        ),
        "PreGstResponsiveCrash": (
            'asyncRecoveryPhase = "Eligible"',
            "node \\in Responsive \\cap up",
            "generation[node] < MaxGeneration",
            'asyncRecoveryPhase\' = "RestartRequired"',
            "asyncRecoveryNode' = node",
            "asyncRecoveryGeneration' = generation[node]",
            "asyncRecoveryReplayQueue' = <<>>",
            "UNCHANGED AsyncSchedulerVars",
        ),
        "PreGstResponsiveRestart": (
            'asyncRecoveryPhase = "RestartRequired"',
            "generation[node] = asyncRecoveryGeneration",
            "Restart(node)",
            "UNCHANGED AsyncSchedulerVars",
            'asyncRecoveryPhase\' = "ReplayRequired"',
            "asyncRecoveryGeneration' = generation[node] + 1",
            "asyncRecoveryReplayQueue' = asyncRecoveryReplayQueue",
        ),
        "PreGstResponsiveReplay": (
            "signatures == RestartSignatureReplay(node)",
            "replay == RestartReplay(node)",
            'asyncRecoveryPhase = "ReplayRequired"',
            "NodeIdle(node)",
            "IF Len(signatures) > 0 THEN RecoveryCoreReplay(node, Head(signatures)) ELSE UNCHANGED vars",
            "ResetNodeSchedulerForRestart(node, replay)",
            'IF Len(signatures) > 0 THEN "Replaying" ELSE "Recovered"',
            "IF Len(signatures) > 0 THEN Tail(signatures) ELSE <<>>",
        ),
        "DriveResponsiveReplayHead": (
            "candidate == Head(asyncRecoveryReplayQueue)",
            'asyncRecoveryPhase = "Replaying"',
            "Len(asyncRecoveryReplayQueue) > 0",
            "NodeIdle(node)",
            "RecoveryCoreReplay(node, candidate)",
            "![node] = @ \\o FreshCandidateSequence(candidate)",
            "asyncRecoveryReplayQueue' = Tail(asyncRecoveryReplayQueue)",
            "UNCHANGED AsyncRecoveryLifecycleVars",
        ),
        "FinishResponsiveReplay": (
            "runner == RestartRunnerAssembly(node)",
            'asyncRecoveryPhase = "Replaying"',
            "asyncRecoveryReplayQueue = <<>>",
            "NodeIdle(node)",
            "ReplayCommitSourcesReady(node)",
            "FreshCandidateSequence(runner[1])",
            'asyncRecoveryPhase\' = "Recovered"',
            "asyncRecoveryReplayQueue' = <<>>",
        ),
        "RearmResponsiveRecovery": (
            'asyncRecoveryPhase = "Recovered"',
            "Responsive \\subseteq up",
            "asyncRecoveryReplayQueue = <<>>",
            'asyncRecoveryPhase\' = "Eligible"',
            "asyncRecoveryNode' = 0",
            "asyncRecoveryGeneration' = 0",
            "asyncRecoveryReplayQueue' = <<>>",
            "UNCHANGED <<vars, AsyncSchedulerVars>>",
        ),
        "AsyncNonCrashStep": (
            "DriveResponsiveReplayHead",
            "FinishResponsiveReplay",
            "RearmResponsiveRecovery",
        ),
        "AsyncNext": (
            "PreGstResponsiveCrash(node)",
            "PreGstResponsiveRestart",
            "PreGstResponsiveReplay",
        ),
        "AsyncRecoveryTypeInvariant": (
            "asyncRecoveryPhase \\in AsyncRecoveryPhases",
            "AsyncQueueTyped(asyncRecoveryReplayQueue)",
            "Len(asyncRecoveryReplayQueue) <= 2",
            'candidate.class = "Completion"',
            'candidate.kind \\in {"SignProposal", "SignVote", "SignTimeout"}',
            "candidate.node = asyncRecoveryNode",
            "CandidateConsumerCurrent(candidate)",
            "SequenceSet(RestartSignatureReplay(asyncRecoveryNode))",
            'asyncRecoveryPhase # "Replaying" => asyncRecoveryReplayQueue = <<>>',
            'asyncRecoveryPhase = "Replaying"',
            "~NodeHasApplication(asyncRecoveryNode)",
            "asyncIngressReady[asyncRecoveryNode] = <<>>",
            "request.source # asyncRecoveryNode",
            "ResponsiveReplayScheduledCandidates(asyncRecoveryNode)",
        ),
        "RestartHighestPrepareQCs": (
            "highestRank[node] # NoRank",
            "qc.context = context",
            'qc.phase = "Prepare"',
            "qc.view = highestRank[node]",
            "qc.subject = highestSubject[node]",
        ),
        "RestartDecisionQCs": (
            "decision.qc",
            "entry \\in decisions",
            "entry.node = node",
            "entry.qc.context = context",
        ),
        "RestartLastInstalledTCs": (
            "tc \\in RestartInstalledTCs(node)",
            "other \\in RestartInstalledTCs(node)",
            "other.view <= tc.view",
        ),
        "RestartHighestPrepareControl": (
            "certificates == RestartHighestPrepareQCs(node)",
            "IF certificates = {} THEN {}",
            "QcOutbox(node, CHOOSE qc \\in certificates: TRUE)",
        ),
        "RestartDecisionControl": (
            "certificates == RestartDecisionQCs(node)",
            "IF certificates = {} THEN {}",
            "QcOutbox(node, CHOOSE qc \\in certificates: TRUE)",
        ),
        "RestartLastTCControl": (
            "certificates == RestartLastInstalledTCs(node)",
            "IF certificates = {} THEN {}",
            "TcOutbox(node, CHOOSE tc \\in certificates: TRUE)",
        ),
        "RestartRetainedControl": (
            "{item \\in asyncRetainedControl: item.source # node}",
            "RememberedControl(cleared, RestartHighestPrepareControl(node))",
            "RememberedControl(withPrepare, RestartDecisionControl(node))",
            "RememberedControl(withDecision, RestartLastTCControl(node))",
        ),
        "ResetNodeSchedulerForRestart": (
            "asyncCommandQueues' = [asyncCommandQueues EXCEPT ![node] = <<>>]",
            "asyncNextCommandClass' = [asyncNextCommandClass EXCEPT ![node] = \"Completion\"]",
            "asyncFifoOwed' = [asyncFifoOwed EXCEPT ![node] = FALSE]",
            "asyncRunnerPhase' = [asyncRunnerPhase EXCEPT ![node] = \"Local\"]",
            "asyncRunnerBudget' = [asyncRunnerBudget EXCEPT ![node] = AsyncQueueCapacity]",
            "asyncIoQueues' = [asyncIoQueues EXCEPT ![node] = <<>>]",
            "asyncOutstandingWork' = [asyncOutstandingWork EXCEPT ![node] = {}]",
            "asyncDeferredCompletionQueues' = [asyncDeferredCompletionQueues EXCEPT ![node] = <<>>]",
            "asyncDeferredProgressQueues' = [asyncDeferredProgressQueues EXCEPT ![node] = <<>>]",
            "asyncDeferredNormalQueues' = [asyncDeferredNormalQueues EXCEPT ![node] = <<>>]",
            "asyncCausalQueues' = [asyncCausalQueues EXCEPT ![node] = replay]",
            "asyncOutstandingTags' = [asyncOutstandingTags EXCEPT ![node] = {}]",
            "asyncSentItems' = asyncSentItems",
            "asyncRetainedControl' = RestartRetainedControl(node)",
            "asyncActiveRequests' = {item \\in asyncActiveRequests: item.source # node}",
            "asyncTransport' = asyncTransport",
            "asyncIngressLanes' = [asyncIngressLanes EXCEPT ![node] = [source \\in AsyncIngressSources |-> <<>>]]",
            "asyncIngressReady' = [asyncIngressReady EXCEPT ![node] = <<>>]",
            "asyncHeldChunks' = {receipt \\in asyncHeldChunks: receipt.node # node}",
        ),
        "CommitCertificateDiscoveryDue": (
            "~ResponsiveReplayQuarantined(node)",
        ),
        "TimeoutDue": ("~ResponsiveReplayQuarantined(node)",),
        "RetransmitDue": ("~ResponsiveReplayQuarantined(node)",),
        "AdmitHiddenPacket": ("~ResponsiveReplayQuarantined(recipient)",),
        "CoalesceHiddenPacket": ("~ResponsiveReplayQuarantined(recipient)",),
        "EnqueueIoLocalControl": ("~ResponsiveReplayQuarantined(node)",),
    }
    if (formal_dir / "proof_coverage.json").is_file():
        required_body_tokens.update({
            "DeliveryClass": (
                "HistoricalLockedCommitItem(item)",
                '"TimeoutVote"',
                '"CertifiedResponse"',
                '"CommitCertificateResponse"',
                'THEN "Progress"',
            ),
            "DeferredProgressAfter": (
                "SameProtectedProgressSlotIndices(node, command)",
                "Append(queue, command)",
            ),
            "AsyncConfiguration": (
                "AsyncDeferredProgressCapacity >= 2 * N + 3",
                "AsyncIngressCapacity >= 3 * N + 1",
                "AsyncValidTimeoutVoteWireByteBound <= AsyncTimeoutVoteByteReserve",
            ),
        })
    for symbol, tokens in required_body_tokens.items():
        extracted = _top_level_operator_body(
            source, symbol, preserve_string_contents=True
        )
        if extracted is None:
            errors.append(f"{path}: missing source-fidelity operator {symbol}")
            continue
        body, line = extracted
        normalized = " ".join(body.split())
        missing = [token for token in tokens if token not in normalized]
        if missing:
            errors.append(
                f"{path}:{line}: {symbol} omits required production behavior {missing}"
            )

    scheduler_tuple = _top_level_operator_body(source, "AsyncSchedulerVars")
    reset_scheduler = _top_level_operator_body(
        source, "ResetNodeSchedulerForRestart", preserve_string_contents=True
    )
    if scheduler_tuple is not None and reset_scheduler is not None:
        scheduler_match = re.fullmatch(
            r"\s*<<(.+)>>\s*", scheduler_tuple[0], re.DOTALL
        )
        scheduler_fields = (
            set()
            if scheduler_match is None
            else {
                field.strip()
                for field in scheduler_match.group(1).split(",")
            }
        )
        reset_writes = set(
            re.findall(r"\b([A-Za-z][A-Za-z0-9_]*)'\s*=", reset_scheduler[0])
        )
        if not scheduler_fields or reset_writes != scheduler_fields:
            errors.append(
                f"{path}:{reset_scheduler[1]}: ResetNodeSchedulerForRestart "
                "must write every and only AsyncSchedulerVars component; "
                f"missing={sorted(scheduler_fields - reset_writes)}, "
                f"unexpected={sorted(reset_writes - scheduler_fields)}"
            )

    fairness = _top_level_operator_body(
        source, "AsyncFairnessAt", preserve_string_contents=True
    )
    if fairness is not None:
        normalized_fairness = " ".join(fairness[0].split())
        recovery_fairness_clauses = (
            "WF_AsyncAllVars(PreGstResponsiveRestart)",
            "WF_AsyncAllVars(PreGstResponsiveReplay)",
            "WF_AsyncAllVars(ResponsiveReplayRunNode)",
            "WF_AsyncAllVars(ResponsiveReplayServiceIoWorker)",
            "WF_AsyncAllVars(DriveResponsiveReplayHead)",
            "WF_AsyncAllVars(FinishResponsiveReplay)",
        )
        invalid_counts = {
            clause: normalized_fairness.count(clause)
            for clause in recovery_fairness_clauses
            if normalized_fairness.count(clause) != 1
        }
        if invalid_counts:
            errors.append(
                f"{path}:{fairness[1]}: AsyncFairnessAt must contain exactly "
                "one weak-fair clause for every restart/replay service action; "
                f"counts={invalid_counts}"
            )

    decision_fetch = _top_level_operator_body(
        source, "ExecuteDecisionFetch", preserve_string_contents=True
    )
    if decision_fetch is not None:
        decision_fetch_body, decision_fetch_line = decision_fetch
        forbidden_decision_fetch = (
            "StoreBody(",
            "ValidateBody(",
            "ValidateDecidedBody(",
            "ApplyDecision(",
            "FetchCertifiedBody(",
        )
        present_forbidden = [
            token for token in forbidden_decision_fetch if token in decision_fetch_body
        ]
        if present_forbidden:
            errors.append(
                f"{path}:{decision_fetch_line}: ExecuteDecisionFetch may only "
                "resolve the durable catalog or open certified recovery; "
                f"prohibited eager work {present_forbidden}"
            )

    runtime_step = _top_level_operator_body(
        source, "RuntimeStep", preserve_string_contents=True
    )
    if runtime_step is None:
        errors.append(f"{path}: missing source-fidelity operator RuntimeStep")
    elif "DirectCommitCertificateDiscoveryStep" in runtime_step[0]:
        errors.append(
            f"{path}:{runtime_step[1]}: commit-certificate discovery is an "
            "outer-loop prefix and may not satisfy fair RuntimeStep service"
        )

    discovery_due = _top_level_operator_body(
        source, "CommitCertificateDiscoveryDue", preserve_string_contents=True
    )
    if discovery_due is not None:
        body, line = discovery_due
        forbidden_guards = (
            "asyncDeferredDrainOwed",
            "asyncFifoOwed",
            "NodeQueueNonempty",
        )
        present = [guard for guard in forbidden_guards if guard in body]
        if present:
            errors.append(
                f"{path}:{line}: discovery due may not add model-only runtime "
                f"debt guards {present}; production pairs discovery with a "
                "separate executor turn"
            )

    runner_path = (
        formal_dir.parents[2]
        / "crates"
        / "iroha_core"
        / "src"
        / "sumeragi"
        / "v2_runner.rs"
    )
    if not runner_path.is_file():
        errors.append(
            f"{runner_path}: production runner is required to refine discovery "
            "as an auxiliary prefix"
        )
    else:
        runner_source = runner_path.read_text(encoding="utf-8")
        drive_definition = runner_source.find("\nfn drive_block_sync(")
        main_loop = (
            runner_source
            if drive_definition < 0
            else runner_source[:drive_definition]
        )
        drive_call = main_loop.rfind("drive_block_sync(")
        drain_call = main_loop.rfind("drain_v2_ingress(")
        if drive_call < 0 or drain_call < 0 or drive_call >= drain_call:
            errors.append(
                f"{runner_path}: the live-height loop must run drive_block_sync "
                "before drain_v2_ingress"
            )

        drain_start = runner_source.find("fn drain_v2_ingress(")
        drain_end = runner_source.find("\nfn outer_ingress_turns(", drain_start)
        drain_body = (
            ""
            if drain_start < 0 or drain_end < 0
            else runner_source[drain_start:drain_end]
        )
        turn_loop = drain_body.find("for turn in outer_ingress_turns(limit)")
        runtime_turn = drain_body.find(
            "if turn == OuterIngressTurn::Runtime", turn_loop
        )
        executor_turn = drain_body.find(
            "advance_executor(executor, services, 1)?", runtime_turn
        )
        ingress_receive = drain_body.find("receiver.try_recv_if", executor_turn)
        if not (
            0 <= turn_loop < runtime_turn < executor_turn < ingress_receive
        ):
            errors.append(
                f"{runner_path}: every outer ingress occurrence must be preceded "
                "by one serialized advance_executor turn"
            )

        outer_start = runner_source.find("fn outer_ingress_turns(")
        outer_end = runner_source.find("\nfn v2_ingress_head_can_drain(", outer_start)
        outer_body = (
            ""
            if outer_start < 0 or outer_end < 0
            else " ".join(runner_source[outer_start:outer_end].split())
        )
        expected_turns = (
            "(0..limit.max(1)).flat_map(|_| "
            "[OuterIngressTurn::Runtime, OuterIngressTurn::Ingress])"
        )
        if expected_turns not in outer_body:
            errors.append(
                f"{runner_path}: outer_ingress_turns must keep the exact "
                "Runtime-before-Ingress alternation"
            )

    adapter_path = (
        formal_dir.parents[2]
        / "crates"
        / "iroha_core"
        / "src"
        / "sumeragi"
        / "v2.rs"
    )
    if not adapter_path.is_file():
        errors.append(
            f"{adapter_path}: production adapter is required to refine "
            "causal admission debt"
        )
    else:
        adapter_source = adapter_path.read_text(encoding="utf-8")
        step_start = adapter_source.find("fn step_with_defer_policy(")
        step_end = adapter_source.find("\n    fn record_ingress_delivery(", step_start)
        step_body = (
            ""
            if step_start < 0 or step_end < 0
            else adapter_source[step_start:step_end]
        )
        reducer_step = step_body.find("let outcome = self.reducer.step(event)?;")
        causal_drive = step_body.find(
            "self.drive_effects(outcome.into_effects())?", reducer_step
        )
        if not (0 <= reducer_step < causal_drive):
            errors.append(
                f"{adapter_path}: reducer effects must enter drive_effects "
                "synchronously before outer ingress resumes"
            )

        drive_start = adapter_source.find("fn drive_effects(")
        drive_end = adapter_source.find("\n    fn convert_effect(", drive_start)
        drive_body = (
            ""
            if drive_start < 0 or drive_end < 0
            else adapter_source[drive_start:drive_end]
        )
        drive_markers = (
            "while let Some(effect) = pending.pop_front()",
            "self.wal.append(&payload)",
            "let persisted = reducer::Event::Persisted { tag, id }",
            "self.reducer.step(persisted.clone())",
            "pending.push_front(effect)",
        )
        positions = [drive_body.find(marker) for marker in drive_markers]
        if any(position < 0 for position in positions) or positions != sorted(
            positions
        ):
            errors.append(
                f"{adapter_path}: drive_effects must synchronously consume "
                "WAL Persisted continuations in emitted order"
            )

    effects_path = (
        formal_dir.parents[2]
        / "crates"
        / "iroha_core"
        / "src"
        / "sumeragi"
        / "v2_effects.rs"
    )
    if not effects_path.is_file():
        errors.append(
            f"{effects_path}: production effect executor is required to "
            "refine causal admission debt"
        )
    else:
        effects_source = effects_path.read_text(encoding="utf-8")
        consume_start = effects_source.find("fn consume_effects")
        consume_end = effects_source.find(
            "\n    /// Consume only the local", consume_start
        )
        consume_body = (
            ""
            if consume_start < 0 or consume_end < 0
            else effects_source[consume_start:consume_end]
        )
        effect_loop = consume_body.find("for effect in effects")
        consume_one = consume_body.find("self.consume_one(effect, services)")
        step_dispatch = effects_source.find(
            "let count = self.consume_effects(effects, services)?;"
        )
        if not (0 <= effect_loop < consume_one and step_dispatch >= 0):
            errors.append(
                f"{effects_path}: every AdapterEffect must be consumed in "
                "vector order before the executor returns"
            )

    regular = _top_level_operator_body(
        source, "RegularCoreCommand", preserve_string_contents=True
    )
    if regular is not None:
        body, line = regular
        validation_branch = re.search(
            r'\\/ /\\ command\.kind = "ValidateBody"(?P<body>.*?)'
            r'(?=\n  \\/ /\\ command\.kind = "BeginPrepare")',
            body,
            re.DOTALL,
        )
        required_validation_tokens = (
            "ValidateBody(command.node, proposal)",
            "RejectBody(command.node, proposal)",
            "DecisionQcValues",
            "ValidateDecidedBody(command.node, qc)",
        )
        if validation_branch is None:
            errors.append(
                f"{path}:{line}: RegularCoreCommand is missing its exact "
                "ValidateBody branch"
            )
        else:
            normalized = " ".join(validation_branch.group("body").split())
            missing = [
                token for token in required_validation_tokens if token not in normalized
            ]
            if missing:
                errors.append(
                    f"{path}:{line}: RegularCoreCommand ValidateBody branch omits "
                    f"required production validation behavior {missing}"
                )
            if 'command.item.kind = "CertifiedResponse"' in normalized:
                errors.append(
                    f"{path}:{line}: local ValidateBody dispatch must rely on the "
                    "exact durable decision and body, not retain a transport response"
                )

    core_path = formal_dir / "SumeragiV2Core.tla"
    if core_path.is_file():
        core_source = core_path.read_text(encoding="utf-8")
        crash = _top_level_operator_body(
            core_source, "Crash", preserve_string_contents=True
        )
        expected_crash_writes = {
            "up",
            "availableBodies",
            "retainedLockedBodies",
            "validatedBodies",
            "invalidBodies",
            "seenProposals",
            "receivedVotes",
            "receivedQCs",
            "receivedTimeoutVotes",
            "receivedTCs",
            "pendingProposal",
            "pendingPrepare",
            "pendingObservePrepare",
            "pendingLockCommit",
            "pendingTimeout",
            "pendingInstallTC",
            "pendingDecision",
            "signProposals",
            "signVotes",
            "signTimeouts",
        }
        if crash is None:
            errors.append(f"{core_path}: missing source-fidelity operator Crash")
        else:
            crash_body, crash_line = crash
            crash_normalized = " ".join(crash_body.split())
            crash_writes = set(
                re.findall(
                    r"\b([A-Za-z][A-Za-z0-9_]*)'\s*=", crash_body
                )
            )
            if crash_writes != expected_crash_writes:
                errors.append(
                    f"{core_path}:{crash_line}: Crash must clear every and "
                    "only node-local volatile reducer component; "
                    f"missing={sorted(expected_crash_writes - crash_writes)}, "
                    f"unexpected={sorted(crash_writes - expected_crash_writes)}"
                )
            crash_reset_tokens = (
                "up' = up \\ {node}",
                "availableBodies' = {body \\in availableBodies: body.node # node}",
                "retainedLockedBodies' = {body \\in retainedLockedBodies: body.node # node}",
                "validatedBodies' = {validation \\in validatedBodies: validation.node # node}",
                "invalidBodies' = {body \\in invalidBodies: body.node # node}",
                "seenProposals' = {entry \\in seenProposals: entry.node # node}",
                "receivedVotes' = {entry \\in receivedVotes: entry.node # node}",
                "receivedQCs' = {entry \\in receivedQCs: entry.node # node}",
                "receivedTimeoutVotes' = {entry \\in receivedTimeoutVotes: entry.node # node}",
                "receivedTCs' = {entry \\in receivedTCs: entry.node # node}",
                "pendingProposal' = {request \\in pendingProposal: request.node # node}",
                "pendingPrepare' = {request \\in pendingPrepare: request.node # node}",
                "pendingObservePrepare' = {request \\in pendingObservePrepare: request.node # node}",
                "pendingLockCommit' = {request \\in pendingLockCommit: request.node # node}",
                "pendingTimeout' = {request \\in pendingTimeout: request.node # node}",
                "pendingInstallTC' = {request \\in pendingInstallTC: request.node # node}",
                "pendingDecision' = {request \\in pendingDecision: request.node # node}",
                "signProposals' = {request \\in signProposals: request.node # node}",
                "signVotes' = {request \\in signVotes: request.node # node}",
                "signTimeouts' = {request \\in signTimeouts: request.node # node}",
            )
            missing_resets = [
                token for token in crash_reset_tokens if token not in crash_normalized
            ]
            if missing_resets:
                errors.append(
                    f"{core_path}:{crash_line}: Crash must reset volatile "
                    f"knowledge only for the crashed node; missing {missing_resets}"
                )
            durable_frame_tokens = (
                "height",
                "context",
                "contextHistory",
                "nodeView",
                "generation",
                "gst",
                "durableBodies",
                "proposalIntents",
                "prepareIntents",
                "commitIntents",
                "timeoutIntents",
                "prepareQCs",
                "commitQCs",
                "formedTCs",
                "installedTCs",
                "lockRank",
                "lockSubject",
                "highestRank",
                "highestSubject",
                "proposalNetwork",
                "voteNetwork",
                "qcNetwork",
                "timeoutNetwork",
                "tcNetwork",
                "decisions",
                "applied",
            )
            unchanged_match = re.search(
                r"UNCHANGED\s*<<(.*?)>>", crash_body, re.DOTALL
            )
            unchanged_fields = (
                set()
                if unchanged_match is None
                else {
                    field.strip()
                    for field in unchanged_match.group(1).split(",")
                }
            )
            missing_durable_frame = sorted(
                set(durable_frame_tokens) - unchanged_fields
            )
            if missing_durable_frame:
                errors.append(
                    f"{core_path}:{crash_line}: Crash may not orphan durable "
                    "intent, certificate, lock, decision, body, or authenticated "
                    f"history state; missing UNCHANGED {missing_durable_frame}"
                )

        restart = _top_level_operator_body(
            core_source, "Restart", preserve_string_contents=True
        )
        if restart is None:
            errors.append(f"{core_path}: missing source-fidelity operator Restart")
        else:
            restart_body, restart_line = restart
            restart_normalized = " ".join(restart_body.split())
            restart_writes = set(
                re.findall(
                    r"\b([A-Za-z][A-Za-z0-9_]*)'\s*=", restart_body
                )
            )
            if restart_writes != {"up", "generation"}:
                errors.append(
                    f"{core_path}:{restart_line}: Restart must write only up "
                    "and the authenticated generation; "
                    f"found {sorted(restart_writes)}"
                )
            restart_tokens = (
                "node \\in ValidatorIds \\ up",
                "generation[node] < MaxGeneration",
                "up' = up \\cup {node}",
                "generation' = [generation EXCEPT ![node] = @ + 1]",
                "durableBodies",
                "proposalIntents",
                "prepareIntents",
                "commitIntents",
                "timeoutIntents",
                "prepareQCs",
                "commitQCs",
                "installedTCs",
                "lockRank",
                "highestRank",
                "decisions",
                "applied",
            )
            missing_restart = [
                token for token in restart_tokens if token not in restart_normalized
            ]
            if missing_restart:
                errors.append(
                    f"{core_path}:{restart_line}: Restart omits authenticated "
                    f"generation or durable-state preservation {missing_restart}"
                )

        core_reconstruction_tokens = {
            "BroadcastVotes": (
                "recipient \\in CurrentVoters \\ {vote.signer}",
            ),
            "CompleteVoteSignature": (
                "receivedVotes' = receivedVotes \\cup "
                "{VoteAt(request.node, request.vote)}",
            ),
            "PersistInstallTC": (
                "signVotes' = signVotes \\cup "
                "ActiveLockedCommitSignRequestsAfterInstall(node, tc)",
            ),
            "VoteResumeAuthorized": (
                'vote.phase = "Prepare"',
                "vote \\in prepareIntents",
                "vote.view = nodeView[node]",
                "~NodeTimedOut(node, vote.view)",
                'vote.phase = "Commit"',
                "vote \\in commitIntents",
                "vote.view <= nodeView[node]",
                "LockedPrepareRound(node, vote.view, vote.subject)",
            ),
        }
        for symbol, tokens in core_reconstruction_tokens.items():
            extracted = _top_level_operator_body(
                core_source, symbol, preserve_string_contents=True
            )
            if extracted is None:
                errors.append(
                    f"{core_path}: missing source-fidelity operator {symbol}"
                )
                continue
            body, line = extracted
            normalized = " ".join(body.split())
            missing = [token for token in tokens if token not in normalized]
            if missing:
                errors.append(
                    f"{core_path}:{line}: {symbol} omits TC vote-pool "
                    f"reconstruction behavior {missing}"
                )

        extracted = _top_level_operator_body(core_source, "DeliverVote")
        if extracted is None:
            errors.append(f"{core_path}: missing source-fidelity operator DeliverVote")
        else:
            body, line = extracted
            normalized = " ".join(body.split())
            required_vote_tokens = (
                "received \\notin receivedVotes",
                "receivedVotes' = receivedVotes \\cup {received}",
                "voteNetwork",
            )
            missing = [
                token for token in required_vote_tokens if token not in normalized
            ]
            if missing:
                errors.append(
                    f"{core_path}:{line}: DeliverVote must retain authenticated "
                    f"history and consume one receipt-pool epoch; missing {missing}"
                )
            if re.search(r"\bvoteNetwork'\s*=", body):
                errors.append(
                    f"{core_path}:{line}: DeliverVote must not consume or rewrite "
                    "immutable authenticated vote history"
                )
        extracted = _top_level_operator_body(core_source, "DeliverTimeout")
        if extracted is None:
            errors.append(
                f"{core_path}: missing source-fidelity operator DeliverTimeout"
            )
        else:
            body, line = extracted
            normalized = " ".join(body.split())
            required_timeout_tokens = (
                "envelope.vote.height = height",
                "TimeoutVoteSlotOccupied(envelope.recipient, envelope.vote)",
                "THEN receivedTimeoutVotes",
                "ELSE receivedTimeoutVotes \\cup {received}",
            )
            missing = [
                token for token in required_timeout_tokens if token not in normalized
            ]
            if missing:
                errors.append(
                    f"{core_path}:{line}: DeliverTimeout omits first-vote-per-signer "
                    f"pool behavior {missing}"
                )

        extracted = _top_level_operator_body(
            core_source, "ValidateDecidedBody", preserve_string_contents=True
        )
        if extracted is None:
            errors.append(
                f"{core_path}: missing certificate-first validation operator "
                "ValidateDecidedBody"
            )
        else:
            body, line = extracted
            normalized = " ".join(body.split())
            required_decided_validation_tokens = (
                "ValidationRecord(node, context, qc.view, generation[node], qc.subject)",
                "decision == [node |-> node, qc |-> qc]",
                "decision \\in decisions",
                'qc.phase = "Commit"',
                "qc.context = context",
                "BodyHeldBy(durableBodies, node, context, qc.view, qc.subject)",
                "qc.subject \\in ValidSubjects",
                "validatedBodies' = validatedBodies \\cup {validation}",
            )
            missing = [
                token
                for token in required_decided_validation_tokens
                if token not in normalized
            ]
            if missing:
                errors.append(
                    f"{core_path}:{line}: ValidateDecidedBody omits exact durable "
                    f"decision validation authority {missing}"
                )
            if "ProposalAt(" in body:
                errors.append(
                    f"{core_path}:{line}: certificate-first validation must not "
                    "fabricate or require leader proposal authority"
                )

        extracted = _top_level_operator_body(core_source, "Next")
        if extracted is not None:
            body, line = extracted
            if "ValidateDecidedBody(node, qc)" not in " ".join(body.split()):
                errors.append(
                    f"{core_path}:{line}: Core Next must expose certificate-first "
                    "decision body validation"
                )

    repo_root = formal_dir.parents[2]
    rust_path = repo_root / "crates" / "iroha_core" / "src" / "sumeragi" / "v2.rs"
    if rust_path.is_file():
        rust_source = rust_path.read_text(encoding="utf-8")
        required_rust_tokens = (
            "const fn semantic_ingress_capacity(roster_len: usize) -> usize",
            "let protected_capacity_bypass =",
            "locked_commit_progress || matches!(key, IngressSemanticKey::TimeoutVote { .. })",
            "matches!(key, IngressSemanticKey::TimeoutVote { .. })",
            "if capacity_bypass && !protected_capacity_bypass",
            "let matches_current_timeout = |key: IngressSemanticKey|",
            "matches_current_lock(*key, record.fingerprint) || matches_current_timeout(*key)",
            "semantic_ingress_capacity(self.wire_context.roster.len())",
            "fn capacity_bypass_records_follow_current_lock_and_timeout_view()",
            "roster_len * 2",
            "assert_eq!(adapter.ingress_equivocations, same_view_equivocations)",
            "fn assert_timeout_vote_owner_rolls_back_across_view_and_retries()",
            "for attempt in 0..2",
            "assert_registry_eq(&adapter.registry, &registry_before)",
            "assert!(!adapter.ingress_deliveries.contains_key(&current_key))",
            "fn full_normal_deferred_lane_cannot_drop_absolute_timeout()",
            "assert_timeout_vote_owner_rolls_back_across_view_and_retries();",
            "MAX_INGRESS_SEMANTIC_KEYS",
            ".is_some_and(|record| record.capacity_bypass)",
            "Some(DeferredProgressClass::TimeoutVote)",
        )
        missing = [
            token for token in required_rust_tokens if token not in rust_source
        ]
        if missing:
            errors.append(
                f"{rust_path}: authenticated TimeoutVote admission must retain "
                f"its current-view semantic-capacity bypass; missing {missing}"
            )

    liveness_cfg = formal_dir / "liveness.cfg"
    if liveness_cfg.is_file():
        cfg_source = liveness_cfg.read_text(encoding="utf-8")
        if "INVARIANT ReceivedTimeoutVotePoolInvariant\n" not in cfg_source:
            errors.append(
                f"{liveness_cfg}: timeout-pool uniqueness must remain a TLC invariant"
            )
        if "INVARIANT AsyncProgressOwnershipInvariant\n" not in cfg_source:
            errors.append(
                f"{liveness_cfg}: scheduler progress ownership must remain a TLC invariant"
            )
        if "INVARIANT AsyncRecoveryTypeInvariant\n" not in cfg_source:
            errors.append(
                f"{liveness_cfg}: responsive recovery state must remain a TLC invariant"
            )
        if "INVARIANT AsyncRestartAuthorityInvariant\n" not in cfg_source:
            errors.append(
                f"{liveness_cfg}: responsive restart authority must remain a TLC invariant"
            )
    crash_replay_configs = (
        "crash_replay_signature_fixed.cfg",
        "crash_replay_body_fixed.cfg",
        "crash_replay_application_fixed.cfg",
        "crash_replay_signature_drop_bug.cfg",
        "crash_replay_body_drop_bug.cfg",
        "crash_replay_application_drop_bug.cfg",
        "crash_replay_stale_completion_bug.cfg",
    )
    for config_name in crash_replay_configs:
        config_path = formal_dir / config_name
        if not config_path.is_file():
            continue
        config_source = config_path.read_text(encoding="utf-8")
        for invariant in (
            "AsyncRecoveryTypeInvariant",
            "AsyncRestartAuthorityInvariant",
        ):
            if f"INVARIANT {invariant}\n" not in config_source:
                errors.append(
                    f"{config_path}: crash/replay TLC search must retain "
                    f"{invariant}"
                )
    return errors


def _ownership_n1_configuration_errors(formal_dir: Path) -> list[str]:
    """Keep the one-validator ownership search on the exact protected geometry."""

    path = formal_dir / "ownership_n1.cfg"
    if not path.is_file():
        return [f"{path}: missing one-validator ownership configuration"]
    source = path.read_text(encoding="utf-8")
    errors: list[str] = []

    def natural(name: str) -> int | None:
        matches = re.findall(rf"(?m)^  {re.escape(name)} = ([0-9]+)$", source)
        if len(matches) != 1:
            errors.append(
                f"{path}: ownership search must pin {name} exactly once"
            )
            return None
        return int(matches[0])

    validator_count = natural("N")
    ingress_capacity = natural("AsyncIngressCapacity")
    deferred_progress_capacity = natural("AsyncDeferredProgressCapacity")
    if validator_count is not None and validator_count != 1:
        errors.append(f"{path}: ownership search must remain the N=1 boundary")
    if validator_count is not None and ingress_capacity is not None:
        exact_ingress_capacity = 3 * validator_count + 1
        if ingress_capacity != exact_ingress_capacity or ingress_capacity != 4:
            errors.append(
                f"{path}: N=1 AsyncIngressCapacity must equal exact 3 * N + 1 "
                f"geometry (4), found {ingress_capacity}"
            )
    if validator_count is not None and deferred_progress_capacity is not None:
        exact_deferred_capacity = 2 * validator_count + 3
        if (
            deferred_progress_capacity != exact_deferred_capacity
            or deferred_progress_capacity != 5
        ):
            errors.append(
                f"{path}: N=1 AsyncDeferredProgressCapacity must equal exact "
                f"2 * N + 3 geometry (5), found {deferred_progress_capacity}"
            )
    return errors


def _successor_production_source_fidelity_errors(repo_root: Path) -> list[str]:
    """Bind the indexed successor/catch-up actions to production source order."""

    errors: list[str] = []

    def load(relative: str) -> tuple[Path, str]:
        path = repo_root / relative
        if not path.is_file():
            errors.append(f"{path}: missing production successor-refinement source")
            return path, ""
        return path, path.read_text(encoding="utf-8")

    def region(
        path: Path,
        source: str,
        label: str,
        start_marker: str,
        end_marker: str,
    ) -> str:
        start = source.find(start_marker)
        end = source.find(end_marker, start + len(start_marker)) if start >= 0 else -1
        if start < 0 or end < 0:
            errors.append(f"{path}: missing exact production region {label}")
            return ""
        return source[start:end]

    def require_tokens(path: Path, label: str, body: str, tokens: tuple[str, ...]) -> None:
        normalized = " ".join(body.split())
        missing = [token for token in tokens if token not in normalized]
        if missing:
            errors.append(
                f"{path}: {label} omits production refinement tokens {missing}"
            )

    def require_order(path: Path, label: str, body: str, markers: tuple[str, ...]) -> None:
        positions = [body.find(marker) for marker in markers]
        if any(position < 0 for position in positions) or positions != sorted(positions):
            errors.append(
                f"{path}: {label} must preserve exact production order {markers}"
            )

    runner_path, runner_source = load(
        "crates/iroha_core/src/sumeragi/v2_runner.rs"
    )
    if runner_source:
        construction = region(
            runner_path,
            runner_source,
            "PendingSuccessorConstruction",
            "impl PendingSuccessorConstruction {",
            "/// One-shot ownership of an authenticated successor's activation handoff.",
        )
        require_tokens(
            runner_path,
            "PendingSuccessorConstruction",
            construction,
            (
                "super::status::begin_v2_successor_activation(finalized_height)?;",
                "PendingSuccessorActivation::Applied { finalized_height: self.finalized_height, successor_context_id, }",
            ),
        )
        activation = region(
            runner_path,
            runner_source,
            "PendingSuccessorActivation",
            "impl PendingSuccessorActivation {",
            "#[derive(Clone, Copy, Debug, PartialEq, Eq)]",
        )
        require_tokens(
            runner_path,
            "PendingSuccessorActivation",
            activation,
            (
                "Self::Recovered { finalized_height, successor_context_id, }",
                "super::status::activate_v2_successor_height( finalized_height, successor_context_id, successor, )?;",
                "super::status::activate_recovered_v2_successor_height( finalized_height, successor_context_id, successor, )?;",
            ),
        )
        open_ingress = region(
            runner_path,
            runner_source,
            "open_ingress_for_active_height",
            "fn open_ingress_for_active_height(",
            "\nfn ingress_capacity_error(",
        )
        require_order(
            runner_path,
            "open_ingress_for_active_height",
            open_ingress,
            (
                "block_ingress.open()",
                "ingress_ready.store(true, Ordering::Release)",
                "activation.publish(successor)",
                "close_ingress_for_rollover(ingress_ready, block_ingress)",
            ),
        )
        run_inner = region(
            runner_path,
            runner_source,
            "run_inner",
            "fn run_inner(",
            "\nfn replayed_proposal_sign_tag(",
        )
        require_tokens(
            runner_path,
            "run_inner recovery ownership",
            run_inner,
            (
                "let recovered_successor_activation_parent = recovered.successor_activation_parent();",
                "PendingSuccessorActivation::recovered(parent, verified_context.context().id())",
            ),
        )
        require_order(
            runner_path,
            "run_inner live successor startup",
            run_inner,
            (
                "SumeragiV2Adapter::open_deferred_status(",
                "SerializedV2Runtime::new(",
                "V2EffectExecutor::open(",
                "ProductionV2Services::start(",
                "executor.consume_effects(startup_effects, &mut services)?",
                "executor.arm_live_clocks(height_started_at)?",
                "successor_activation_status_snapshot()",
                "open_ingress_for_active_height(",
            ),
        )
        require_order(
            runner_path,
            "run_inner applied successor handoff",
            run_inner,
            (
                "PendingSuccessorConstruction::begin(receipt.height())?",
                "build_verified_successor(",
                "activation.bind(verified_context.context().id())",
            ),
        )
        require_tokens(
            runner_path,
            "historical ingress routing",
            region(
                runner_path,
                runner_source,
                "drain_v2_ingress",
                "fn drain_v2_ingress(",
                "\n#[derive(Clone, Copy, Debug, PartialEq, Eq)]\nenum OuterIngressTurn",
            ),
            (
                "block_sync_server.serve_historical_body( kura, context_store, request, &sender, local_key, )",
                "executor.accept_certified_body_response(response, &sender, services)",
                "block_sync.authenticate_response(response, &sender)",
                "block_sync.enqueue_and_complete(discovered, |message| { executor.enqueue_network(message).map(|_| ()) })",
            ),
        )

    status_path, status_source = load(
        "crates/iroha_core/src/sumeragi/status.rs"
    )
    if status_source:
        begin = region(
            status_path,
            status_source,
            "begin_v2_successor_activation",
            "pub(crate) fn begin_v2_successor_activation(",
            "\nfn validate_v2_successor_snapshot(",
        )
        require_tokens(
            status_path,
            "begin_v2_successor_activation",
            begin,
            (
                "SumeragiV2LocalWorkStage::Queued",
                "SumeragiV2LocalWorkStage::Running",
            ),
        )
        validate = region(
            status_path,
            status_source,
            "validate_v2_successor_snapshot",
            "fn validate_v2_successor_snapshot(",
            "\nfn activate_v2_successor_height_at(",
        )
        require_tokens(
            status_path,
            "validate_v2_successor_snapshot",
            validate,
            (
                "finalized_height.checked_add(1)",
                "successor.last_committed_height != finalized_height",
                "successor.height_context_id != expected_successor_context_id",
                "marker.round.context_id == successor.height_context_id",
                "marker.transition == SumeragiV2ProgressTransition::SuccessorHeightActivated",
                "marker.age_ms == 0",
            ),
        )
        applied = region(
            status_path,
            status_source,
            "activate_v2_successor_height_at",
            "fn activate_v2_successor_height_at(",
            "\nfn activate_recovered_v2_successor_height_at(",
        )
        require_order(
            status_path,
            "activate_v2_successor_height_at",
            applied,
            (
                "validate_v2_successor_snapshot(",
                "SumeragiV2LocalWorkStage::Running",
                "SumeragiV2LocalWorkStage::Complete",
                "set_v2_status_at(successor, now)",
            ),
        )
        recovered = region(
            status_path,
            status_source,
            "activate_recovered_v2_successor_height_at",
            "fn activate_recovered_v2_successor_height_at(",
            "\n/// Publish the exact one-shot boundary",
        )
        require_order(
            status_path,
            "activate_recovered_v2_successor_height_at",
            recovered,
            (
                "validate_v2_successor_snapshot(",
                "if let Some(published)",
                "set_v2_status_at(successor, now)",
            ),
        )
        if "update_v2_successor_work_stage_at(" in recovered:
            errors.append(
                f"{status_path}: recovered successor publication may not fabricate "
                "physical predecessor completion"
            )

    adapter_path, adapter_source = load(
        "crates/iroha_core/src/sumeragi/v2.rs"
    )
    if adapter_source:
        deferred_open = region(
            adapter_path,
            adapter_source,
            "open_deferred_status",
            "pub(crate) fn open_deferred_status(",
            "\n    #[allow(clippy::too_many_arguments)]\n    fn open_with_aggregator(",
        )
        require_tokens(
            adapter_path,
            "open_deferred_status",
            deferred_open,
            ("Self::open_with_aggregator_and_publication(", "false,"),
        )
        marker = region(
            adapter_path,
            adapter_source,
            "successor_activation_status",
            "pub(crate) fn successor_activation_status(",
            "\n    fn liveness_status(",
        )
        require_order(
            adapter_path,
            "successor_activation_status",
            marker,
            (
                "SumeragiV2ProgressTransition::SuccessorHeightActivated",
                "self.status()",
            ),
        )

    runtime_path, runtime_source = load(
        "crates/iroha_core/src/sumeragi/v2_runtime.rs"
    )
    if runtime_source:
        snapshot = region(
            runtime_path,
            runtime_source,
            "successor_activation_status_snapshot",
            "pub(crate) fn successor_activation_status_snapshot(",
            "\n    fn body_pipeline_completion_is_owned(",
        )
        require_order(
            runtime_path,
            "successor_activation_status_snapshot",
            snapshot,
            (
                "if !self.clocks_armed",
                "AdapterError::SuccessorClocksNotArmed",
                "self.driver.successor_activation_status()",
            ),
        )

    block_sync_path, block_sync_source = load(
        "crates/iroha_core/src/sumeragi/v2_block_sync.rs"
    )
    if block_sync_source:
        enqueue = region(
            block_sync_path,
            block_sync_source,
            "enqueue_and_complete",
            "pub(crate) fn enqueue_and_complete<",
            "\n    /// Number of bounded outstanding requests.",
        )
        require_order(
            block_sync_path,
            "enqueue_and_complete",
            enqueue,
            (
                "enqueue(discovered.message())",
                "self.complete(discovered)",
            ),
        )
        historical = region(
            block_sync_path,
            block_sync_source,
            "build_historical_body_response",
            "fn build_historical_body_response(",
            "\nfn ensure_key_identity(",
        )
        require_order(
            block_sync_path,
            "build_historical_body_response",
            historical,
            (
                "kura.v2_finality_artifact(height)?",
                "context_store\n        .load(height)?",
                "persisted.context() != &artifact.height_context",
                "authenticate_certified_body_request(",
                "verify_persisted_quorum_certificate(",
                "binary_search(&responder)",
                "kura\n        .get_block(block_height)",
                "block.hash() != request.subject.block_hash",
                "block.canonical_resultless_proposal()",
                "encode_payload(",
                "response.validate_against(",
            ),
        )

    effects_path, effects_source = load(
        "crates/iroha_core/src/sumeragi/v2_effects.rs"
    )
    if effects_source:
        certified = region(
            effects_path,
            effects_source,
            "accept_certified_body_response",
            "pub(crate) fn accept_certified_body_response<",
            "\n    /// Accept a durable application completion",
        )
        require_order(
            effects_path,
            "accept_certified_body_response",
            certified,
            (
                "self.outstanding_requests.authenticate_response(",
                "ReadyBody::derive(",
                "self.plan_fetch_completion(",
                "services.complete_certified_body_fetch(",
                "self.commit_fetch_completion(plan)",
            ),
        )
        consume = region(
            effects_path,
            effects_source,
            "consume_one",
            "fn consume_one<",
            "\n    fn ensure_pending_tip_recovery_effect_is_local(",
        )
        require_order(
            effects_path,
            "consume_one body pipeline",
            consume,
            (
                "AdapterEffect::FetchBody",
                "AdapterEffect::StoreBody",
                "AdapterEffect::ValidateBody",
                "AdapterEffect::Apply",
            ),
        )

    release_path, release_source = load(
        "scripts/run_sumeragi_v2_release_gates.sh"
    )
    if release_source:
        for test in (
            "sumeragi::v2_block_sync::tests::discovery_outputs_only_normal_commit_qc_ingress_and_waits_for_enqueue",
            "sumeragi::v2_block_sync::tests::catch_up_is_strictly_sequential_across_contexts",
            "sumeragi::v2_block_sync::tests::historical_body_comes_from_kura_and_only_a_certified_signer_can_serve",
            "sumeragi::v2_runtime::tests::successor_activation_snapshot_requires_armed_live_clocks",
            "sumeragi::v2_runner::tests::successor_activation_is_published_only_after_ingress_is_open",
            "sumeragi::v2_runner::tests::complete_tip_recovery_uses_the_same_live_successor_boundary",
            "sumeragi::v2_runner::tests::successor_startup_failure_stays_running_and_fails_closed_without_activation",
        ):
            if release_source.count(f"  {test}\n") != 1:
                errors.append(
                    f"{release_path}: production refinement test must be pinned exactly once: {test}"
                )
    return errors


def _chain_source_fidelity_errors(formal_dir: Path) -> list[str]:
    """Keep chain composition per-node and independent of the old global barrier."""

    chain_path = formal_dir / "SumeragiV2ChainEpoch.tla"
    proof_path = formal_dir / "SumeragiV2ChainEpochProofs.tla"
    refinement_path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    errors: list[str] = []
    scheduler_fields = (
        "asyncNow",
        "asyncCommandQueues",
        "asyncNextCommandClass",
        "asyncFifoOwed",
        "asyncTimeoutEmitted",
        "asyncRunnerPhase",
        "asyncRunnerBudget",
        "asyncCausalAdmissionOwed",
        "asyncNextLocalSource",
        "asyncIoQueues",
        "asyncOutstandingWork",
        "asyncIoReadyCompletions",
        "asyncLocalReadyCompletions",
        "asyncNextCompletionSource",
        "asyncIoControlAvailable",
        "asyncDeferredCompletionQueues",
        "asyncDeferredProgressQueues",
        "asyncDeferredNormalQueues",
        "asyncNextDeferredClass",
        "asyncDeferredDrainOwed",
        "asyncCausalQueues",
        "asyncOutstandingTags",
        "asyncNodeDeadlines",
        "asyncRetransmitDeadlines",
        "asyncNodeServiceDeadlines",
        "asyncIoServiceDeadlines",
        "asyncSentItems",
        "asyncRetainedControl",
        "asyncActiveRequests",
        "asyncTransport",
        "asyncIngressLanes",
        "asyncIngressReady",
        "asyncHeldChunks",
    )
    scheduler_arity = len(scheduler_fields)
    recovery_fields = (
        "asyncRecoveryPhase",
        "asyncRecoveryNode",
        "asyncRecoveryGeneration",
        "asyncRecoveryReplayQueue",
    )
    recovery_arity = len(recovery_fields)
    node_service_deadline_slot = scheduler_fields.index(
        "asyncNodeServiceDeadlines"
    ) + 1

    async_path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    if async_path.is_file():
        async_source = async_path.read_text(encoding="utf-8")
        async_scheduler = _top_level_operator_body(
            async_source, "AsyncSchedulerVars"
        )
        if async_scheduler is None:
            errors.append(f"{async_path}: missing AsyncSchedulerVars")
        else:
            body, line = async_scheduler
            tuple_match = re.fullmatch(r"\s*<<(.+)>>\s*", body, re.DOTALL)
            actual_scheduler_fields = (
                ()
                if tuple_match is None
                else tuple(
                    field.strip()
                    for field in tuple_match.group(1).split(",")
                )
            )
            if actual_scheduler_fields != scheduler_fields:
                errors.append(
                    f"{async_path}:{line}: AsyncSchedulerVars must match the "
                    "chain projection's exact ordered scheduler tuple; found "
                    f"{actual_scheduler_fields!r}"
                )
        async_recovery = _top_level_operator_body(async_source, "AsyncRecoveryVars")
        if async_recovery is None:
            errors.append(f"{async_path}: missing AsyncRecoveryVars")
        else:
            body, line = async_recovery
            tuple_match = re.fullmatch(r"\s*<<(.+)>>\s*", body, re.DOTALL)
            actual_recovery_fields = (
                ()
                if tuple_match is None
                else tuple(
                    field.strip() for field in tuple_match.group(1).split(",")
                )
            )
            if actual_recovery_fields != recovery_fields:
                errors.append(
                    f"{async_path}:{line}: AsyncRecoveryVars must match the "
                    "chain projection's exact ordered recovery tuple; found "
                    f"{actual_recovery_fields!r}"
                )
        async_all_vars = _top_level_operator_body(async_source, "AsyncAllVars")
        expected_async_all_vars = "<<vars, AsyncSchedulerVars, AsyncRecoveryVars>>"
        if async_all_vars is None:
            errors.append(f"{async_path}: missing AsyncAllVars")
        else:
            body, line = async_all_vars
            normalized = " ".join(body.split())
            if normalized != expected_async_all_vars:
                errors.append(
                    f"{async_path}:{line}: AsyncAllVars must equal only "
                    f"{expected_async_all_vars!r}; found {normalized!r}"
                )

    if chain_path.is_file():
        raw_chain_source = chain_path.read_text(encoding="utf-8")
        source = strip_tla_comments(raw_chain_source)
        header = re.search(r"(?m)^EXTENDS\s+(.+)$", source)
        extended_modules = (
            set()
            if header is None
            else {module.strip() for module in header.group(1).split(",")}
        )
        if "SumeragiV2Core" not in extended_modules:
            errors.append(
                f"{chain_path}: chain/epoch state must extend SumeragiV2Core directly"
            )
        if re.search(r"\bSumeragiV2Reconfiguration\b", source):
            errors.append(
                f"{chain_path}: chain/epoch state may not inherit the global "
                "application-barrier model"
            )

        required_body_tokens = {
            "RecordCertifiedNext": (
                "certifiedHeight' = nextHeight",
                "UNCHANGED <<nodeHeight, nodeContext",
            ),
            "RecordAppliedNext": (
                "node == application.node",
                "nodeHeight[node]",
                "![node] = nextHeight",
                "![node] = ContextRecord(nextHeight, nextLineage)",
            ),
        }
        for symbol, tokens in required_body_tokens.items():
            extracted = _top_level_operator_body(source, symbol)
            if extracted is None:
                errors.append(f"{chain_path}: missing per-node chain operator {symbol}")
                continue
            body, line = extracted
            normalized = " ".join(body.split())
            missing = [token for token in tokens if token not in normalized]
            if missing:
                errors.append(
                    f"{chain_path}:{line}: {symbol} omits required per-node "
                    f"chain behavior {missing}"
                )
            for forbidden in ("CommonAppliedSubject", "AdvanceContext", "NextV2"):
                if re.search(rf"\b{forbidden}\b", body):
                    errors.append(
                        f"{chain_path}:{line}: {symbol} may not use global-barrier "
                        f"operator {forbidden}"
                    )

        tlc_harness_contracts = {
            "ChainEpochNext": (
                "\\/ \\E decision \\in DecisionEvidenceSet: "
                "RecordCertifiedNext(decision) \\/ \\E decision \\in "
                "DecisionEvidenceSet: RecordKnownDecision(decision) "
                "\\/ \\E application \\in DecisionEvidenceSet: "
                "RecordAppliedNext(application) \\/ \\E application \\in "
                "DecisionEvidenceSet: RecordKnownApplication(application)"
            ),
            "ChainEpochSpec": (
                "ChainEpochInit /\\ [][ChainEpochNext]_ChainEpochVars"
            ),
            "CandidateHistoricalCommitCertificateSet": (
                '{QC(qcContext, roundView, "Commit", subject, signers): '
                "qcContext \\in ContextRecords, roundView \\in Views, "
                "subject \\in ValidSubjects, signers \\in SUBSET ValidatorIds}"
            ),
            "HistoricalCommitCertificateSet": (
                "{qc \\in CandidateHistoricalCommitCertificateSet: "
                "DualQuorum(qc.context.epoch, qc.signers)}"
            ),
            "CandidateDurableDecisionEvidenceSet": (
                "{[node |-> node, qc |-> qc]: node \\in ValidatorIds, "
                "qc \\in HistoricalCommitCertificateSet}"
            ),
            "DurableDecisionEvidenceSet": (
                "{decision \\in CandidateDurableDecisionEvidenceSet: "
                "decision \\in DecisionEvidenceSet}"
            ),
            "ChainEpochTlcVars": "<<vars, ChainEpochVars>>",
            "ChainEpochTlcInit": "Init /\\ ChainEpochInit",
            "ChainEpochTlcReceiptNext": (
                "\\/ \\E decision \\in DurableDecisionEvidenceSet: "
                "RecordCertifiedNext(decision) \\/ \\E decision \\in "
                "DurableDecisionEvidenceSet: RecordKnownDecision(decision) "
                "\\/ \\E application \\in DurableDecisionEvidenceSet: "
                "RecordAppliedNext(application) \\/ \\E application \\in "
                "DurableDecisionEvidenceSet: RecordKnownApplication(application)"
            ),
            "ChainEpochTlcNext": (
                "ChainEpochTlcReceiptNext /\\ UNCHANGED vars"
            ),
            "ChainEpochTlcSpec": (
                "ChainEpochTlcInit /\\ [][ChainEpochTlcNext]_ChainEpochTlcVars"
            ),
            "ChainEpochTlcInvariant": "TypeInvariant /\\ ChainEpochInvariant",
        }
        for symbol, exact_body in tlc_harness_contracts.items():
            extracted = _top_level_operator_body(
                raw_chain_source, symbol, preserve_string_contents=True
            )
            if extracted is None:
                errors.append(
                    f"{chain_path}: missing full-state TLC chain harness {symbol}"
                )
                continue
            body, line = extracted
            normalized = " ".join(body.split())
            if normalized != exact_body:
                errors.append(
                    f"{chain_path}:{line}: {symbol} must equal only "
                    f"{exact_body!r}; found {normalized!r}"
                )

    if proof_path.is_file():
        proof_source = proof_path.read_text(encoding="utf-8")
        property_contracts = {
            "ChainPrefixProperty": (
                "specification => [](/\\ HistoryPrefixComparable "
                "/\\ NodeAppliedPrefixBacked)"
            ),
            "EpochBoundaryProperty": (
                "specification => [](/\\ PerNodeFrozenEpoch "
                "/\\ PerNodeParentFinality /\\ ForeignLineageRejected "
                "/\\ ForeignContextCertificateRejected)"
            ),
        }
        for symbol, exact_body in property_contracts.items():
            extracted = _top_level_operator_body(proof_source, symbol)
            if extracted is None:
                errors.append(f"{proof_path}: missing stable chain property {symbol}")
                continue
            body, line = extracted
            normalized = " ".join(body.split())
            if normalized != exact_body:
                errors.append(
                    f"{proof_path}:{line}: {symbol} must equal only "
                    f"{exact_body!r}; found {normalized!r}"
                )
        refinement = _top_level_theorem_body(
            proof_source, "ChainEpochTlcReceiptNextRefinesChainEpochNext"
        )
        exact_refinement = "ChainEpochTlcReceiptNext => ChainEpochNext"
        if refinement is None:
            errors.append(
                f"{proof_path}: missing checked TLC-to-deductive receipt refinement"
            )
        else:
            body, line = refinement
            statement = re.split(
                r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b", body, maxsplit=1
            )[0]
            normalized = " ".join(statement.split())
            if normalized != exact_refinement:
                errors.append(
                    f"{proof_path}:{line}: TLC receipt refinement must state only "
                    f"{exact_refinement!r}; found {normalized!r}"
                )

    if refinement_path.is_file():
        raw_source = refinement_path.read_text(encoding="utf-8")
        source = strip_tla_comments(raw_source)
        retired_shadows = (
            "asyncCertifiedHeight",
            "asyncDecidedAt",
            "asyncNodeHeight",
            "asyncNodeContext",
            "asyncDurableDecisionEvidence",
            "asyncDurableApplicationEvidence",
            "AsyncHistoryNext",
            "AsyncHistoryVars",
        )
        for symbol in retired_shadows:
            for match in re.finditer(rf"\b{re.escape(symbol)}\b", source):
                line = source.count("\n", 0, match.start()) + 1
                errors.append(
                    f"{refinement_path}:{line}: stale async chain shadow {symbol} "
                    "is prohibited"
                )
        for forbidden in ("CommonAppliedSubject", "AdvanceContext", "NextV2"):
            for match in re.finditer(rf"\b{forbidden}\b", source):
                line = source.count("\n", 0, match.start()) + 1
                errors.append(
                    f"{refinement_path}:{line}: chain refinement may not depend on "
                    f"global-barrier operator {forbidden}"
                )

        indexed_recovery = _top_level_operator_body(
            raw_source, "IndexedRecovery", preserve_string_contents=True
        )
        expected_indexed_recovery = (
            "indexedAsyncState[initialContext][3][component]"
        )
        if indexed_recovery is None:
            errors.append(f"{refinement_path}: missing IndexedRecovery projection")
        else:
            body, line = indexed_recovery
            normalized = " ".join(body.split())
            if normalized != expected_indexed_recovery:
                errors.append(
                    f"{refinement_path}:{line}: IndexedRecovery must equal only "
                    f"{expected_indexed_recovery!r}; found {normalized!r}"
                )

        indexed_async = _top_level_operator_body(raw_source, "IndexedAsync")
        indexed_async_normalized: str | None = None
        if indexed_async is None:
            errors.append(
                f"{refinement_path}: missing indexed production-network instance"
            )
        else:
            body, line = indexed_async
            normalized = " ".join(body.split())
            indexed_async_normalized = normalized
            if "INSTANCE SumeragiV2AsyncNetwork WITH" not in normalized:
                errors.append(
                    f"{refinement_path}:{line}: IndexedAsync must directly "
                    "instantiate the authoritative SumeragiV2AsyncNetwork"
                )
            core_fields = (
                "height",
                "context",
                "contextHistory",
                "nodeView",
                "generation",
                "up",
                "gst",
                "availableBodies",
                "durableBodies",
                "retainedLockedBodies",
                "validatedBodies",
                "invalidBodies",
                "seenProposals",
                "receivedVotes",
                "receivedQCs",
                "receivedTimeoutVotes",
                "receivedTCs",
                "proposalIntents",
                "prepareIntents",
                "commitIntents",
                "timeoutIntents",
                "prepareQCs",
                "commitQCs",
                "formedTCs",
                "installedTCs",
                "lockRank",
                "lockSubject",
                "highestRank",
                "highestSubject",
                "pendingProposal",
                "pendingPrepare",
                "pendingObservePrepare",
                "pendingLockCommit",
                "pendingTimeout",
                "pendingInstallTC",
                "pendingDecision",
                "signProposals",
                "signVotes",
                "signTimeouts",
                "proposalNetwork",
                "voteNetwork",
                "qcNetwork",
                "timeoutNetwork",
                "tcNetwork",
                "decisions",
                "applied",
            )
            expected_core_mappings = tuple(
                f"{field} <- IndexedCore(initialContext, {index})"
                for index, field in enumerate(core_fields, start=1)
            )
            missing_core = [
                mapping
                for mapping in expected_core_mappings
                if mapping not in normalized
            ]
            if missing_core:
                errors.append(
                    f"{refinement_path}:{line}: IndexedAsync Core tuple mapping "
                    f"does not match vars; missing {missing_core}"
                )
            expected_mappings = tuple(
                f"{field} <- IndexedScheduler(initialContext, {index})"
                for index, field in enumerate(scheduler_fields, start=1)
            )
            missing = [
                mapping for mapping in expected_mappings if mapping not in normalized
            ]
            if missing:
                errors.append(
                    f"{refinement_path}:{line}: IndexedAsync scheduler tuple mapping "
                    f"does not match AsyncSchedulerVars; missing {missing}"
                )
            expected_recovery_mappings = tuple(
                f"{field} <- IndexedRecovery(initialContext, {index})"
                for index, field in enumerate(recovery_fields, start=1)
            )
            missing_recovery = [
                mapping
                for mapping in expected_recovery_mappings
                if mapping not in normalized
            ]
            if missing_recovery:
                errors.append(
                    f"{refinement_path}:{line}: IndexedAsync recovery tuple mapping "
                    "does not match AsyncRecoveryVars; missing "
                    f"{missing_recovery}"
                )

        verification_context = re.search(
            r"(?m)^CONSTANTS?[ \t]+VerificationContext[ \t]*$", source
        )
        if verification_context is None:
            errors.append(
                f"{refinement_path}: missing proof-only VerificationContext constant"
            )

        verification_helpers = {
            "VerificationCore": "IndexedCore(VerificationContext, component)",
            "VerificationScheduler": (
                "IndexedScheduler(VerificationContext, component)"
            ),
            "VerificationRecovery": (
                "IndexedRecovery(VerificationContext, component)"
            ),
        }
        for symbol, expected_body in verification_helpers.items():
            extracted = _top_level_operator_body(
                raw_source, symbol, preserve_string_contents=True
            )
            if extracted is None:
                errors.append(
                    f"{refinement_path}: missing proof-only {symbol} mapping"
                )
                continue
            helper_body, helper_line = extracted
            helper_normalized = " ".join(helper_body.split())
            if helper_normalized != expected_body:
                errors.append(
                    f"{refinement_path}:{helper_line}: {symbol} must equal only "
                    f"{expected_body!r}; found {helper_normalized!r}"
                )

        verification_async_proof = _top_level_operator_body(
            raw_source, "VerificationAsyncProof"
        )
        if verification_async_proof is None:
            errors.append(
                f"{refinement_path}: missing proof-only VerificationAsyncProof instance"
            )
        else:
            proof_body, proof_line = verification_async_proof
            proof_normalized = " ".join(proof_body.split())
            proof_prefix = "INSTANCE SumeragiV2AsyncLivenessProofs WITH"
            network_prefix = "INSTANCE SumeragiV2AsyncNetwork WITH"
            if proof_prefix not in proof_normalized:
                errors.append(
                    f"{refinement_path}:{proof_line}: VerificationAsyncProof "
                    "must directly instantiate SumeragiV2AsyncLivenessProofs"
                )
            elif indexed_async_normalized is not None:
                expected_proof_mapping = indexed_async_normalized.replace(
                    network_prefix, proof_prefix, 1
                )
                expected_proof_mapping = re.sub(
                    r"IndexedCore\(initialContext,\s*",
                    "VerificationCore(",
                    expected_proof_mapping,
                )
                expected_proof_mapping = re.sub(
                    r"IndexedScheduler\(initialContext,\s*",
                    "VerificationScheduler(",
                    expected_proof_mapping,
                )
                expected_proof_mapping = re.sub(
                    r"IndexedRecovery\(initialContext,\s*",
                    "VerificationRecovery(",
                    expected_proof_mapping,
                )
                if proof_normalized != expected_proof_mapping:
                    errors.append(
                        f"{refinement_path}:{proof_line}: "
                        "VerificationAsyncProof must use the exact IndexedAsync "
                        "Core/scheduler/recovery tuple substitution through the "
                        "VerificationCore, VerificationScheduler, and "
                        "VerificationRecovery mappings"
                    )

        indexed_shape = _top_level_operator_body(
            raw_source, "IndexedAsyncStateShape"
        )
        if indexed_shape is None:
            errors.append(f"{refinement_path}: missing IndexedAsyncStateShape")
        else:
            body, line = indexed_shape
            normalized = " ".join(body.split())
            required = (
                "Len(indexedAsyncState[initialContext]) = 3",
                "Len(indexedAsyncState[initialContext][1]) = 46",
                f"Len(indexedAsyncState[initialContext][2]) = {scheduler_arity}",
                f"Len(indexedAsyncState[initialContext][3]) = {recovery_arity}",
            )
            missing = [token for token in required if token not in normalized]
            if missing:
                errors.append(
                    f"{refinement_path}:{line}: IndexedAsyncStateShape has stale "
                    f"Core/scheduler/recovery tuple arity {missing}"
                )

        exact_variables = _top_level_theorem_body(
            raw_source, "IndexedInstanceVariablesAreExact"
        )
        exact_variables_statement = (
            "IndexedAsyncStateShape => \\A initialContext \\in "
            "AdmissibleContextRecords: IndexedAsync(initialContext)!AsyncAllVars "
            "= IndexedAsyncStateAt(initialContext)"
        )
        if exact_variables is None:
            errors.append(
                f"{refinement_path}: missing IndexedInstanceVariablesAreExact"
            )
        else:
            body, line = exact_variables
            statement = re.split(
                r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b", body, maxsplit=1
            )[0]
            normalized_statement = " ".join(statement.split())
            if normalized_statement != exact_variables_statement:
                errors.append(
                    f"{refinement_path}:{line}: "
                    "IndexedInstanceVariablesAreExact must state only "
                    f"{exact_variables_statement!r}; found "
                    f"{normalized_statement!r}"
                )
            missing_definitions = [
                definition
                for definition in (
                    "IndexedAsyncStateShape",
                    "IndexedAsyncStateAt",
                    "IndexedCore",
                    "IndexedScheduler",
                    "IndexedRecovery",
                )
                if definition not in body
            ]
            if missing_definitions:
                errors.append(
                    f"{refinement_path}:{line}: "
                    "IndexedInstanceVariablesAreExact must unfold every exact "
                    f"tuple projection; missing {missing_definitions}"
                )

        joined_non_runner = _top_level_operator_body(
            raw_source, "IndexedJoinedNonRunnerStep"
        )
        if joined_non_runner is None:
            errors.append(
                f"{refinement_path}: missing IndexedJoinedNonRunnerStep"
            )
        else:
            body, line = joined_non_runner
            normalized = " ".join(body.split())
            direct_discovery_branch = (
                "\\/ \\E node \\in IndexedAsync(initialContext)! "
                "AsyncCurrentResponsiveVoters: "
                "/\\ IndexedNodeCurrentAt(initialContext, node) "
                "/\\ IndexedAsync(initialContext)! "
                "DirectCommitCertificateDiscoveryStep(node)"
            )
            if direct_discovery_branch not in normalized:
                errors.append(
                    f"{refinement_path}:{line}: indexed non-runner step must "
                    "restrict the exact DirectCommitCertificateDiscoveryStep "
                    "to the node's current joined context"
                )
            expected_frame = (
                "UNCHANGED IndexedScheduler(initialContext, "
                f"{node_service_deadline_slot})"
            )
            if expected_frame not in normalized:
                errors.append(
                    f"{refinement_path}:{line}: indexed non-runner frame must "
                    f"preserve scheduler slot {node_service_deadline_slot} "
                    "(asyncNodeServiceDeadlines)"
                )

        discovery_step = _top_level_operator_body(
            raw_source, "IndexedCommitCertificateDiscoveryStep"
        )
        expected_discovery_step = (
            "/\\ IndexedChainNext "
            "/\\ IndexedNodeCurrentAt(initialContext, node) "
            "/\\ IndexedAsync(initialContext)! "
            "PostGstCommitCertificateDiscovery(node)"
        )
        if discovery_step is None:
            errors.append(
                f"{refinement_path}: missing indexed current Commit-certificate "
                "discovery fairness action"
            )
        else:
            body, line = discovery_step
            normalized = " ".join(body.split())
            if normalized != expected_discovery_step:
                errors.append(
                    f"{refinement_path}:{line}: "
                    "IndexedCommitCertificateDiscoveryStep must equal only the "
                    f"current exact discovery product step; found {normalized!r}"
                )

        indexed_fairness = _top_level_operator_body(raw_source, "IndexedFairness")
        exact_discovery_fairness = (
            "WF_IndexedChainVars( "
            "IndexedCommitCertificateDiscoveryStep( initialContext, node))"
        )
        if indexed_fairness is None:
            errors.append(f"{refinement_path}: missing IndexedFairness")
        else:
            body, line = indexed_fairness
            normalized = " ".join(body.split())
            if normalized.count(exact_discovery_fairness) != 1:
                errors.append(
                    f"{refinement_path}:{line}: IndexedFairness must contain "
                    "exactly one weak-fair current Commit-certificate discovery "
                    "product clause"
                )

        # Small unit-test fixtures exercise only the tuple projection above.
        # The production refinement is distinguished by its authoritative
        # indexed product action; once that action exists, every explicit
        # successor and catch-up contract below is mandatory.
        if _top_level_operator_body(raw_source, "IndexedProductActionAt") is None:
            return errors

        def require_chain_operator(
            symbol: str,
            *,
            required: tuple[str, ...] = (),
            forbidden: tuple[str, ...] = (),
            exact: str | None = None,
        ) -> str | None:
            extracted = _top_level_operator_body(
                raw_source, symbol, preserve_string_contents=True
            )
            if extracted is None:
                errors.append(
                    f"{refinement_path}: missing explicit chain operator {symbol}"
                )
                return None
            operator_body, operator_line = extracted
            operator_normalized = " ".join(operator_body.split())
            if exact is not None and operator_normalized != exact:
                errors.append(
                    f"{refinement_path}:{operator_line}: {symbol} must equal only "
                    f"{exact!r}; found {operator_normalized!r}"
                )
            missing_tokens = [
                token for token in required if token not in operator_normalized
            ]
            if missing_tokens:
                errors.append(
                    f"{refinement_path}:{operator_line}: {symbol} omits exact "
                    f"successor/catch-up behavior {missing_tokens}"
                )
            present_forbidden = [
                token for token in forbidden if token in operator_normalized
            ]
            if present_forbidden:
                errors.append(
                    f"{refinement_path}:{operator_line}: {symbol} contains "
                    f"prohibited successor/catch-up behavior {present_forbidden}"
                )
            return operator_normalized

        require_chain_operator(
            "SuccessorActivationRequiredPrerequisites",
            exact=(
                '{"DeferredStatus", "AdapterReady", "RuntimeReady", '
                '"ServicesReady", "StartupApplied", "ClocksArmed", '
                '"IngressOpen"}'
            ),
        )
        require_chain_operator(
            "QueueSuccessorActivation",
            required=(
                'successorActivationStatus[parentContext][node] = "Idle"',
                'successorPredecessorStatusOwnership[parentContext][node] = "Absent"',
                '![parentContext][node] = "Queued"',
                '![parentContext][node] = "Published"',
                "UNCHANGED <<successorActivationTokens,",
            ),
            forbidden=("joinedByContext'",),
        )
        require_chain_operator(
            "IndexedApplicationReceiptHandoff",
            required=(
                "Chain!RecordAppliedNext(application)",
                "QueueSuccessorActivation(initialContext, application.node)",
                "UNCHANGED joinedByContext",
                "Chain!RecordKnownApplication(application)",
            ),
            forbidden=("joinedByContext'",),
        )
        require_chain_operator(
            "ExactSuccessorActivationToken",
            required=(
                "successorContext = CanonicalIndexedContext(parentContext.height + 1)",
                "SuccessorActivationToken( kind, parentContext, node, successorContext) \\in successorActivationTokens",
            ),
            forbidden=("successorContext.height =",),
        )
        require_chain_operator(
            "SuccessorActivationMarker",
            required=(
                "parentContext |-> parentContext",
                "successorContext |-> successorContext",
                "successorHeight |-> successorContext.height",
                "generation |-> 0",
                "view |-> 0",
                'transition |-> "SuccessorHeightActivated"',
            ),
        )
        require_chain_operator(
            "BeginSuccessorActivation",
            required=(
                'successorActivationStatus[parentContext][node] = "Queued"',
                'successorPredecessorStatusOwnership[parentContext][node] = "Published"',
                '![parentContext][node] = "Running"',
                "ExactDurableParentApplication(parentContext, node, application)",
            ),
        )
        require_chain_operator(
            "BindAppliedSuccessorActivationToken",
            required=(
                '"Applied", parentContext, node, successorContext',
                "successorContext = CanonicalIndexedContext(parentContext.height + 1)",
                "ExactDurableParentApplication(parentContext, node, application)",
            ),
        )

        phase_contracts = {
            "OpenDeferredSuccessorAdapter": (
                "successorActivationPrerequisites[parentContext][node] = {}",
                "SuccessorActivationAdapterPrerequisites",
            ),
            "ConstructSuccessorRuntime": (
                "= SuccessorActivationAdapterPrerequisites",
                "SuccessorActivationRuntimePrerequisites",
            ),
            "StartSuccessorServices": (
                "= SuccessorActivationRuntimePrerequisites",
                "SuccessorActivationServicePrerequisites",
            ),
            "ApplySuccessorStartupEffects": (
                "= SuccessorActivationServicePrerequisites",
                "SuccessorActivationStartupPrerequisites",
            ),
            "ArmSuccessorClocks": (
                "= SuccessorActivationStartupPrerequisites",
                "SuccessorActivationClockPrerequisites",
            ),
            "PrepareSuccessorActivationMarker": (
                "= SuccessorActivationClockPrerequisites",
                "marker \\notin preparedSuccessorActivationMarkers",
            ),
            "OpenSuccessorIngress": (
                "= SuccessorActivationClockPrerequisites",
                "marker \\in preparedSuccessorActivationMarkers",
                "SuccessorActivationRequiredPrerequisites",
            ),
        }
        for symbol, tokens in phase_contracts.items():
            require_chain_operator(
                symbol,
                required=(
                    "SuccessorActivationCredentialReady(",
                    *tokens,
                ),
            )

        require_chain_operator(
            "FailClosedSuccessorStartup",
            required=(
                'successorPredecessorStatusOwnership[parentContext][node] = "Published"',
                '![parentContext][node] = "Queued"',
                '![parentContext][node] = "Absent"',
                "![parentContext][node] = {}",
                "{token \\in successorActivationTokens:",
                "{authority \\in successorRecoveryAuthorities:",
                "{marker \\in preparedSuccessorActivationMarkers:",
                "successorActivationFailureHistory \\cup {owner}",
                "UNCHANGED <<publishedSuccessorActivationMarkers, successorActivationCompletions, joinedByContext>>",
            ),
            forbidden=("joinedByContext'",),
        )
        require_chain_operator(
            "AuthenticateRecoveredSuccessorActivation",
            required=(
                '"Recovered", parentContext, node, successorContext',
                'successorPredecessorStatusOwnership[parentContext][node] = "Absent"',
                "owner \\in successorActivationFailures",
                "ExactDurableParentApplication(parentContext, node, application)",
                "CompleteTipRecoveryAuthorityRecord(",
                "successorRecoveryAuthorities \\cup {authority}",
            ),
            forbidden=('"Applied", parentContext, node, successorContext',),
        )
        require_chain_operator(
            "ExactCompleteTipRecoveryAuthority",
            required=(
                "ExactDurableParentApplication(parentContext, node, application)",
                "successorContext = CanonicalIndexedContext(parentContext.height + 1)",
                "CompleteTipRecoveryAuthorityRecord(",
                "\\in successorRecoveryAuthorities",
            ),
        )
        require_chain_operator(
            "ActivateAppliedSuccessorHeight",
            required=(
                'ExactSuccessorActivationToken( "Applied", parentContext, node, successorContext)',
                'successorPredecessorStatusOwnership[parentContext][node] = "Published"',
                'successorActivationStatus[parentContext][node] = "Running"',
                "= SuccessorActivationRequiredPrerequisites",
                "marker \\in preparedSuccessorActivationMarkers",
                '![parentContext][node] = "Complete"',
                '![parentContext][node] = "Absent"',
                "successorActivationCompletions \\cup {token}",
                "joinedByContext' =",
            ),
        )
        require_chain_operator(
            "ActivateRecoveredSuccessorHeight",
            required=(
                'ExactSuccessorActivationToken( "Recovered", parentContext, node, successorContext)',
                'successorPredecessorStatusOwnership[parentContext][node] = "Absent"',
                "ExactCompleteTipRecoveryAuthority(",
                "= SuccessorActivationRequiredPrerequisites",
                "marker \\in preparedSuccessorActivationMarkers",
                "UNCHANGED successorActivationStatus",
                "successorActivationCompletions \\cup {token}",
                "joinedByContext' =",
            ),
            forbidden=(
                '"Applied", parentContext, node, successorContext',
                '![parentContext][node] = "Complete"',
            ),
        )
        join_writes = len(re.findall(r"joinedByContext'\s*=", raw_source))
        if join_writes != 2:
            errors.append(
                f"{refinement_path}: exactly the Applied and Recovered "
                f"publication actions may write joinedByContext; found {join_writes} writes"
            )

        require_chain_operator(
            "HistoricalCatchUpTarget",
            required=(
                "node \\in Responsive",
                "nodeHeight[node] = initialContext.height",
                "nodeContext[node] = initialContext",
                "~IndexedProjectedNodeHasApplication(initialContext, node)",
            ),
            forbidden=(
                "initialContext.height < MaxHeight",
                "VotingRoster",
            ),
        )
        require_chain_operator(
            "HistoricalCatchUpSource",
            required=(
                "source \\in durableDecisionEvidence",
                "source \\in durableApplicationEvidence",
                "initialContext.height < MaxHeight",
                "Chain!CanonicalCommitForSlot(",
                "initialContext.height = MaxHeight",
                "Chain!ReceiptOutsideChainHorizon(source)",
                "server \\in source.qc.signers \\cap Honest",
                "BodyHeldBy(",
            ),
        )
        require_chain_operator(
            "HistoricalCatchUpShape",
            required=(
                '{"Idle", "DecisionRecovered", "BodyRecovered", "Stored", "Validated", "Applied"}',
            ),
        )
        historical_stage_contracts = {
            "IndexedHistoricalCatchUpDecision": (
                '= "Idle"',
                '= "DecisionRecovered"',
                "Chain!RecordKnownDecision(decision)",
            ),
            "IndexedHistoricalCatchUpBodyRecovery": (
                '= "DecisionRecovered"',
                '= "BodyRecovered"',
            ),
            "IndexedHistoricalCatchUpBodyStore": (
                '= "BodyRecovered"',
                '= "Stored"',
            ),
            "IndexedHistoricalCatchUpValidation": (
                '= "Stored"',
                '= "Validated"',
            ),
        }
        for symbol, tokens in historical_stage_contracts.items():
            require_chain_operator(
                symbol,
                required=(
                    "HistoricalCatchUpSource(initialContext, server, source)",
                    *tokens,
                ),
            )
        require_chain_operator(
            "IndexedHistoricalCatchUpNonterminalApplication",
            required=(
                "initialContext.height < MaxHeight",
                '= "Validated"',
                "Chain!RecordAppliedNext(application)",
                "QueueSuccessorActivation(initialContext, node)",
                '= "Applied"',
                "UNCHANGED <<historicalCatchUpDecisions, indexedAsyncState, joinedByContext>>",
            ),
            forbidden=("joinedByContext'",),
        )
        require_chain_operator(
            "IndexedHistoricalCatchUpTerminalApplication",
            required=(
                "initialContext.height = MaxHeight",
                "Chain!ReceiptOutsideChainHorizon(application)",
                '= "Validated"',
                "Chain!RecordKnownApplication(application)",
                '= "Applied"',
                "UNCHANGED <<historicalCatchUpDecisions, indexedAsyncState, joinedByContext, SuccessorActivationVars>>",
            ),
            forbidden=(
                "Chain!RecordAppliedNext(application)",
                "QueueSuccessorActivation",
                "joinedByContext'",
            ),
        )
        require_chain_operator(
            "IndexedHistoricalCatchUpPipelineAction",
            required=(
                "IndexedHistoricalCatchUpDecision(",
                "IndexedHistoricalCatchUpBodyRecovery(",
                "IndexedHistoricalCatchUpBodyStore(",
                "IndexedHistoricalCatchUpValidation(",
                "IndexedHistoricalCatchUpApplication(",
            ),
        )
        if indexed_fairness is not None:
            fairness_normalized = " ".join(indexed_fairness[0].split())
            catch_up_fairness = (
                "WF_IndexedChainVars( "
                "IndexedCatchUpPipelineStep(initialContext, node))"
            )
            activation_fairness = (
                "WF_IndexedChainVars( "
                "IndexedSuccessorActivationProgressStep( initialContext, node))"
            )
            if fairness_normalized.count(catch_up_fairness) != 1:
                errors.append(
                    f"{refinement_path}:{indexed_fairness[1]}: IndexedFairness "
                    "must contain exactly one fair staged historical catch-up pipeline"
                )
            if fairness_normalized.count(activation_fairness) != 1:
                errors.append(
                    f"{refinement_path}:{indexed_fairness[1]}: IndexedFairness "
                    "must contain exactly one fair successor-activation pipeline"
                )
    errors.extend(_successor_production_source_fidelity_errors(ROOT_DIR))
    return errors


def _retired_path_present(path: Path) -> bool:
    """Treat an empty, untracked legacy directory as absent."""

    if path.is_dir() and not path.is_symlink():
        return any(path.iterdir())
    return path.exists()


def _nightly_chaos_cold_cache_errors(repo_root: Path) -> list[str]:
    """Pin the online prefetch/offline chaos boundary for a cold Cargo cache."""

    harness_path = repo_root / "scripts" / "formal" / "run_sumeragi_v2_harness.sh"
    lock_path = repo_root / "scripts" / "formal" / "sumeragi_v2_harness.lock"
    launcher_path = repo_root / "scripts" / "run_sumeragi_v2_100k_chaos.sh"
    workflow_path = repo_root / ".github" / "workflows" / "nightly_sumeragi_formal.yml"
    required_paths = (harness_path, lock_path, launcher_path, workflow_path)
    errors = [
        f"{path}: missing cold-cache chaos contract input"
        for path in required_paths
        if not path.is_file() or path.is_symlink()
    ]
    if errors:
        return errors

    harness = harness_path.read_text(encoding="utf-8")
    lock_declaration = (
        'readonly HARNESS_LOCK="${REPO_ROOT}/scripts/formal/'
        'sumeragi_v2_harness.lock"'
    )
    if harness.count(lock_declaration) != 1:
        errors.append(
            f"{harness_path}: harness must name the pinned standalone lock "
            "exactly once"
        )
    digest_matches = re.findall(
        r'(?m)^readonly HARNESS_LOCK_SHA256="([0-9a-f]{64})"$', harness
    )
    if len(digest_matches) != 1:
        errors.append(
            f"{harness_path}: harness must pin exactly one literal SHA-256 "
            "for the standalone lock"
        )
    else:
        actual_digest = hashlib.sha256(lock_path.read_bytes()).hexdigest()
        if digest_matches[0] != actual_digest:
            errors.append(
                f"{harness_path}: pinned standalone lock digest disagrees "
                f"with {lock_path}"
            )

    normalized_harness = " ".join(harness.split())
    exact_network_mode = (
        'if [[ "$1" == "--fetch" ]]; then export CARGO_NET_OFFLINE=false '
        "else export CARGO_NET_OFFLINE=true fi"
    )
    if exact_network_mode not in normalized_harness:
        errors.append(
            f"{harness_path}: only --fetch may run online and every test mode "
            "must force CARGO_NET_OFFLINE=true"
        )
    lock_validation_tokens = (
        '[[ ! -f "$HARNESS_LOCK" || -L "$HARNESS_LOCK"',
        '"$(hash_file "$HARNESS_LOCK")" != "$HARNESS_LOCK_SHA256"',
    )
    missing_lock_validation = [
        token for token in lock_validation_tokens if token not in normalized_harness
    ]
    if missing_lock_validation:
        errors.append(
            f"{harness_path}: standalone lock validation is incomplete; "
            f"missing {missing_lock_validation}"
        )

    lock_copy = harness.find('cp -- "$HARNESS_LOCK" Cargo.lock')
    case_start = harness.find('case "$1" in')
    fetch_start = harness.find("  --fetch)", case_start)
    unit_start = harness.find("  --unit)", fetch_start)
    if not (0 <= lock_copy < case_start < fetch_start < unit_start):
        errors.append(
            f"{harness_path}: the verified standalone lock must be copied "
            "before dispatching --fetch or any offline test mode"
        )
        fetch_branch = ""
    else:
        fetch_branch = harness[fetch_start:unit_start]
    fetch_commands = re.findall(r"(?m)^\s*cargo fetch[^\n]*$", fetch_branch)
    if fetch_commands != ["    cargo fetch --locked"]:
        errors.append(
            f"{harness_path}: --fetch must perform exactly one online "
            f"`cargo fetch --locked`; found {fetch_commands}"
        )

    chaos_start = harness.find("  --chaos-100k)", unit_start)
    replay_start = harness.find("  --model-replay)", chaos_start)
    chaos_branch = (
        ""
        if chaos_start < 0 or replay_start < 0
        else harness[chaos_start:replay_start]
    )
    chaos_cargo_commands = re.findall(r"(?m)^\s*cargo test\b", chaos_branch)
    offline_chaos_commands = re.findall(
        r"(?m)^\s*cargo test --locked --offline "
        r"-p iroha_sumeragi_core\s*\\?$",
        chaos_branch,
    )
    if len(chaos_cargo_commands) != 2 or len(offline_chaos_commands) != 2:
        errors.append(
            f"{harness_path}: --chaos-100k inventory and execution must both "
            "remain --locked --offline"
        )

    launcher = launcher_path.read_text(encoding="utf-8")
    chaos_invocation = (
        "bash scripts/formal/run_sumeragi_v2_harness.sh --chaos-100k"
    )
    if launcher.count(chaos_invocation) != 1:
        errors.append(
            f"{launcher_path}: source-attested chaos launcher must invoke "
            "the offline harness gate exactly once"
        )

    workflow = workflow_path.read_text(encoding="utf-8")
    job_match = re.search(
        r"(?ms)^  sumeragi-v2-chaos-100k:\n(?P<body>.*?)"
        r"(?=^  [A-Za-z0-9_-]+:\n|\Z)",
        workflow,
    )
    if job_match is None:
        errors.append(
            f"{workflow_path}: missing independent sumeragi-v2-chaos-100k job"
        )
    else:
        job = job_match.group("body")
        cache_marker = "- uses: Swatinem/rust-cache@v2"
        fetch_marker = (
            "run: bash scripts/formal/run_sumeragi_v2_harness.sh --fetch"
        )
        gate_marker = "run: bash scripts/run_sumeragi_v2_100k_chaos.sh"
        counts = {
            "cache": job.count(cache_marker),
            "fetch": job.count(fetch_marker),
            "source_attested_gate": job.count(gate_marker),
        }
        if counts != {"cache": 1, "fetch": 1, "source_attested_gate": 1}:
            errors.append(
                f"{workflow_path}: nightly chaos job must contain exactly one "
                f"cache, pinned prefetch, and source-attested gate; counts={counts}"
            )
        elif not (
            job.index(cache_marker)
            < job.index(fetch_marker)
            < job.index(gate_marker)
        ):
            errors.append(
                f"{workflow_path}: nightly --fetch must run after cache restore "
                "and before the source-attested chaos gate"
            )
    return errors


def _release_evidence_errors(
    ledger: dict[str, Any],
    evidence: dict[str, Any] | None,
    *,
    formal_dir: Path = FORMAL_DIR,
    root_dir: Path = ROOT_DIR,
) -> list[str]:
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

    if evidence is None:
        return errors + ["release gate requires fresh TLAPS proof evidence"]
    if not isinstance(evidence, dict):
        return errors + ["proof evidence must be a JSON object"]
    expected_top_level_keys = {
        "schema_version",
        "protocol",
        "backend_verification",
        "tool",
        "source_manifest",
        "modules",
    }
    if set(evidence) != expected_top_level_keys:
        errors.append(
            "proof evidence fields must equal "
            f"{sorted(expected_top_level_keys)}, found {sorted(evidence)}"
        )
    if evidence.get("schema_version") != EVIDENCE_SCHEMA_VERSION:
        errors.append(f"proof evidence schema_version must equal {EVIDENCE_SCHEMA_VERSION}")
    if evidence.get("protocol") != "sumeragi-v2":
        errors.append("proof evidence protocol must equal sumeragi-v2")
    if evidence.get("backend_verification") is not True:
        errors.append("release gate requires backend-verified TLAPS evidence")

    tool = evidence.get("tool")
    if not isinstance(tool, dict):
        errors.append("proof evidence tool must be an object")
    else:
        if set(tool) != {"name", "commit", "version"}:
            errors.append("proof evidence tool fields must be name, commit, and version")
        if tool.get("name") != "TLAPM":
            errors.append("proof evidence must identify TLAPM")
        if tool.get("commit") != TLAPM_COMMIT:
            errors.append(f"proof evidence must use pinned TLAPM commit {TLAPM_COMMIT}")
        version = tool.get("version")
        if version != TLAPM_COMMIT[:7]:
            errors.append(
                f"proof evidence TLAPM version must equal {TLAPM_COMMIT[:7]}"
            )

    expected_manifest = _formal_source_manifest(formal_dir, root_dir)
    if evidence.get("source_manifest") != expected_manifest:
        errors.append("proof evidence source manifest does not match current TLA+ sources")
    source_manifest_sha256 = expected_manifest["sha256"]

    modules = evidence.get("modules")
    if not isinstance(modules, list):
        errors.append("proof evidence modules must be an array")
        return errors
    observed: list[str] = []
    for entry in modules:
        if not isinstance(entry, dict):
            errors.append("proof evidence module entries must be objects")
            continue
        if set(entry) != {
            "module",
            "obligations_proved",
            "log",
            "log_sha256",
            "source_manifest_sha256",
        }:
            errors.append("proof evidence module fields are not canonical")
        module = entry.get("module")
        proved = entry.get("obligations_proved")
        if not _nonempty_string(module):
            errors.append("proof evidence module is missing a name")
            continue
        if module not in RELEASE_PROOF_MODULES:
            errors.append(f"proof evidence contains unknown module {module!r}")
            continue
        if module in observed:
            errors.append(f"proof evidence repeats module {module}")
        observed.append(module)
        if not isinstance(proved, int) or isinstance(proved, bool) or proved <= 0:
            errors.append(f"proof evidence module {module} has no positive proved count")
        if entry.get("source_manifest_sha256") != source_manifest_sha256:
            errors.append(
                f"proof evidence module {module} is not bound to the current source manifest"
            )

        log_value = entry.get("log")
        expected_log = f"target/formal/sumeragi_v2/tlaps/{module}.log"
        if log_value != expected_log:
            errors.append(f"proof evidence module {module} must use log {expected_log}")
            continue
        log_path = root_dir / expected_log
        if not log_path.is_file() or log_path.is_symlink():
            errors.append(f"proof evidence log is not a regular file: {log_path}")
            continue
        actual_log_sha256 = _sha256_file(log_path)
        if entry.get("log_sha256") != actual_log_sha256:
            errors.append(f"proof evidence log digest mismatch for {module}")
            continue
        try:
            log_source = log_path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            errors.append(f"proof evidence log is not UTF-8: {log_path}")
            continue
        actual_count = _tlapm_obligation_count(
            log_source,
            module=module,
            source_manifest_sha256=source_manifest_sha256,
        )
        if actual_count is None:
            errors.append(
                f"proof evidence log lacks the exact manifest-bound successful suffix for {module}"
            )
        if actual_count != proved:
            errors.append(f"proof evidence proved count does not match log for {module}")
    if observed != list(RELEASE_PROOF_MODULES):
        errors.append(
            "proof evidence must cover the release proof modules in canonical order; "
            f"expected {list(RELEASE_PROOF_MODULES)}, found {observed}"
        )
    return errors


def validate_ledger(
    ledger: dict[str, Any],
    *,
    formal_dir: Path = FORMAL_DIR,
    verus_source_dir: Path = VERUS_SOURCE_DIR,
    release: bool = False,
    evidence: dict[str, Any] | None = None,
    evidence_root: Path = ROOT_DIR,
    check_retired_paths: bool = True,
) -> LedgerValidation:
    """Validate schema, source linkage, trust boundaries, and release evidence."""

    errors: list[str] = []
    expected_ledger_keys = {
        "schema_version",
        "protocol",
        "status_values",
        "machine_checked_completion",
        "obligations",
    }
    if set(ledger) != expected_ledger_keys:
        errors.append(
            "proof ledger fields must equal "
            f"{sorted(expected_ledger_keys)}, found {sorted(ledger)}; "
            "tool runs and counts belong only in generated proof evidence"
        )
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
    errors.extend(_resume_vote_witness_errors(formal_dir))
    errors.extend(_retired_liveness_errors(formal_dir))
    errors.extend(_bounded_view_dependency_errors(formal_dir))
    errors.extend(_reachable_oracle_guard_errors(formal_dir))
    errors.extend(_generalized_context_init_errors(formal_dir))
    errors.extend(_safety_property_source_fidelity_errors(formal_dir))
    errors.extend(_historical_timeout_derivation_errors(formal_dir))
    errors.extend(_async_spec_shape_errors(formal_dir))
    errors.extend(_async_proof_architecture_errors(formal_dir))
    errors.extend(_progress_witness_source_fidelity_errors(formal_dir))
    errors.extend(_async_source_fidelity_errors(formal_dir))
    errors.extend(_ownership_n1_configuration_errors(formal_dir))
    errors.extend(_chain_source_fidelity_errors(formal_dir))
    errors.extend(_nightly_chaos_cold_cache_errors(ROOT_DIR))
    for cfg_name in REQUIRED_TLC_CONFIGS:
        cfg = formal_dir / cfg_name
        if not cfg.is_file():
            errors.append(f"missing required TLC counterexample configuration: {cfg}")
            continue
        expected_header = REQUIRED_TLC_CONFIG_HEADERS[cfg_name]
        source = cfg.read_text(encoding="utf-8")
        if not source.startswith(expected_header + "\n"):
            errors.append(
                f"{cfg}: TLC configuration must start with {expected_header!r}"
            )
        if '  ValidSubjects = {"A"}\n' not in source:
            errors.append(
                f"{cfg}: TLC configuration must keep B externally invalid so "
                "bounded searches exercise validation rejection"
            )

    obligations = ledger.get("obligations")
    if not isinstance(obligations, list) or not obligations:
        errors.append("proof ledger obligations must be a non-empty array")
        obligations = []
    errors.extend(_proof_obligation_architecture_errors(obligations, module_sources))
    errors.extend(_proof_status_dependency_errors(obligations))
    errors.extend(_proofless_release_theorem_errors(obligations, module_sources))
    specified_unproved = [
        obligation.get("id", f"obligations[{index}]")
        if _nonempty_string(obligation.get("id"))
        else f"obligations[{index}]"
        for index, obligation in enumerate(obligations)
        if isinstance(obligation, dict)
        and obligation.get("status") == "specified_unproved"
    ]
    if completion and specified_unproved:
        errors.append(
            "machine_checked_completion=true rejects specified_unproved "
            f"obligations: {specified_unproved}"
        )
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
            if status == "tlaps_proved" and module not in RELEASE_PROOF_MODULES:
                errors.append(
                    f"{where} claims TLAPS proof in non-release module {module}"
                )
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
                elif not _symbol_exists(
                    source, name, theorem_only=status == "tlaps_proved"
                ):
                    declaration = "theorem" if status == "tlaps_proved" else "symbol"
                    errors.append(
                        f"{where} references missing {declaration} {module}!{name}"
                    )
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
            if _retired_path_present(path):
                errors.append(f"retired Sumeragi v1 formal corridor still exists: {path}")

    if release:
        errors.extend(
            _release_evidence_errors(
                ledger,
                evidence,
                formal_dir=formal_dir,
                root_dir=evidence_root,
            )
        )

    return LedgerValidation(tuple(errors), bool(completion))


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument(
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
        "--evidence",
        type=Path,
        help="fresh proof evidence generated by the pinned TLAPS runner",
    )
    mode.add_argument(
        "--write-evidence",
        type=Path,
        help="write canonical source- and log-bound TLAPS evidence and exit",
    )
    parser.add_argument(
        "--tlapm-version",
        help="full pinned TLAPM version string used with --write-evidence",
    )
    parser.add_argument(
        "--tlaps-log-dir",
        type=Path,
        help="directory containing strict-run logs used with --write-evidence",
    )
    mode.add_argument(
        "--print-proof-modules",
        action="store_true",
        help="print the ordered deductive module list and exit",
    )
    mode.add_argument(
        "--print-source-manifest-sha256",
        action="store_true",
        help="print the current canonical TLA+ source-manifest digest and exit",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    if args.print_proof_modules:
        print("\n".join(RELEASE_PROOF_MODULES))
        return 0
    if args.print_source_manifest_sha256:
        print(_formal_source_manifest()["sha256"])
        return 0
    if args.write_evidence is not None:
        if args.evidence is not None:
            print(
                "--write-evidence cannot be combined with --evidence",
                file=sys.stderr,
            )
            return 2
        if not _nonempty_string(args.tlapm_version) or args.tlaps_log_dir is None:
            print(
                "--write-evidence requires --tlapm-version and --tlaps-log-dir",
                file=sys.stderr,
            )
            return 2
        try:
            evidence = build_release_evidence(
                tlapm_version=args.tlapm_version,
                log_dir=args.tlaps_log_dir,
            )
            args.write_evidence.parent.mkdir(parents=True, exist_ok=True)
            args.write_evidence.write_text(
                json.dumps(evidence, indent=2, ensure_ascii=False) + "\n",
                encoding="utf-8",
            )
        except (OSError, UnicodeDecodeError, ValueError) as error:
            print(f"proof evidence generation failed: {error}", file=sys.stderr)
            return 1
        print(f"wrote Sumeragi v2 proof evidence to {args.write_evidence}")
        return 0
    if args.evidence is not None and not args.release:
        print("--evidence is only valid with --release", file=sys.stderr)
        return 2
    try:
        ledger = load_ledger(args.ledger)
    except (OSError, json.JSONDecodeError, DuplicateKeyError) as error:
        print(f"proof ledger load failed: {error}", file=sys.stderr)
        return 1
    if not isinstance(ledger, dict):
        print("proof ledger must be a JSON object", file=sys.stderr)
        return 1
    evidence: dict[str, Any] | None = None
    if args.release:
        if args.evidence is None:
            print("release gate requires --evidence", file=sys.stderr)
            return 1
        try:
            evidence = load_ledger(args.evidence)
        except (OSError, json.JSONDecodeError, DuplicateKeyError) as error:
            print(f"proof evidence load failed: {error}", file=sys.stderr)
            return 1
        if not isinstance(evidence, dict):
            print("proof evidence must be a JSON object", file=sys.stderr)
            return 1
    result = validate_ledger(ledger, release=args.release, evidence=evidence)
    if result.errors:
        for error in result.errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    if args.release:
        status = "release-complete"
    elif result.machine_checked_completion:
        status = "completion-claimed; release evidence not checked"
    else:
        status = "release-incomplete"
    print(f"Sumeragi v2 proof ledger is structurally valid ({status})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
