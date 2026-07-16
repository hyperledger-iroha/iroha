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
    "chain_epoch.cfg": "SPECIFICATION ChainEpochSpec",
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
# proof, the genesis handoff belongs to the current receipt-driven chain
# product, and multi-height progress belongs there only after it grows an
# indexed family of one-height async instances.
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
    ),
    "height-liveness": (
        "rotating-leader-liveness",
        "application-liveness",
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
        rf"(?m)^{re.escape(symbol)}\s*(?:\([^)=\n]*\))?\s*=="
    ).search(stripped)
    if declaration is None:
        return None
    body_start = declaration.end()
    next_declaration = re.compile(
        r"(?m)^(?:[A-Za-z_][A-Za-z0-9_]*\s*(?:\([^)=\n]*\))?\s*==|"
        r"[ \t]*(?:LOCAL[ \t]+)?"
        r"(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)\b|={4,}\s*$)"
    ).search(stripped, body_start)
    body_end = next_declaration.start() if next_declaration is not None else len(stripped)
    return stripped[body_start:body_end], stripped.count("\n", 0, body_start) + 1


def _top_level_theorem_body(source: str, symbol: str) -> tuple[str, int] | None:
    """Return one top-level theorem body and its first source line."""

    stripped = strip_tla_comments(source)
    declaration = re.compile(
        rf"(?m)^[ \t]*(?:LOCAL[ \t]+)?"
        rf"(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)[ \t]+"
        rf"{re.escape(symbol)}\s*(?:\([^)=\n]*\))?\s*=="
    ).search(stripped)
    if declaration is None:
        return None
    body_start = declaration.end()
    next_declaration = re.compile(
        r"(?m)^(?:[A-Za-z_][A-Za-z0-9_]*\s*(?:\([^)=\n]*\))?\s*==|"
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
    }
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
                r"=> PostGstProgressActionEnabled)"
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
              "StarvationFreedomProperty": (
                r"specification => \A candidate \in AsyncCandidateSet: "
                r"(gst /\ ResponsiveProtectedCandidateOwned(candidate)) ~> "
                r"~ResponsiveProtectedCandidateOwned(candidate)"
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
        "AsyncSetGST": "/\\ ~gst /\\ SetGST /\\ UNCHANGED AsyncSchedulerVars",
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
            "/\\ CommandExecutionEnabled(command) "
            "/\\ (NodeIdle(command.node) "
            "\\/ command.class = \"Completion\")"
        ),
    }
    if (formal_dir / "proof_coverage.json").is_file():
        exact.update({
            "IngressContinuationProtectedSourcesFor": (
                "{source \\in ValidatorIds: "
                "\\/ Len(lanes[recipient][source]) = 0 "
                "\\/ /\\ Len(lanes[recipient][source]) = 1 "
                "/\\ IngressLaneHasProgressIn(lanes, recipient, source)}"
            ),
            "IngressProtectedSlotCountFor": (
                "Cardinality(IngressProtectedSourcesFor(lanes, recipient)) + "
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

    required_body_tokens = {
        "ServiceIoWorker": (
            "asyncIoControlAvailable'",
            "EXCEPT ![node] = TRUE",
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
        "RunNode": ("~NodeHasApplication(node)",),
        "RunHistoricalServer": (
            "NodeHasApplication(node)",
            "DrainHistoricalIngressSelected(node)",
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
        ),
        "IngressItemCanDrain": (
            "CandidateScheduled(candidate)",
            "CanEnqueueClass(node, candidate.class)",
        ),
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
            "PublishCommitCertificateRequests(",
            "CommitCertificateRequestOutbox(node)",
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
        ),
        "DrainHistoricalIngressSelected": (
            "HistoricalSelectedIngressLaneIndex(node, index)",
            "HistoricalSelectedIngressItemAt(node, index)",
            "PopSelectedIngress(node, index, laneIndex)",
        ),
        "AsyncFairnessAt": (
            "PostGstRunNode(node)",
            "PostGstRunHistoricalServer(node)",
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
            'CausalCandidate("Completion", "RequestCertifiedBody", command)',
        ),
    }
    if (formal_dir / "proof_coverage.json").is_file():
        required_body_tokens.update({
            "DeliveryClass": (
                "HistoricalLockedCommitItem(item)",
                'THEN "Progress"',
            ),
            "DeferredProgressAfter": (
                "SameProtectedProgressSlotIndices(node, command)",
                "DominatedProtectedProgressIndices(node, command)",
                "ReplaceableUnprotectedProgressIndices(node)",
            ),
            "AsyncConfiguration": (
                "AsyncDeferredProgressCapacity >= N + 3",
                "AsyncIngressCapacity >= Cardinality(AsyncIngressSources) + Cardinality(ValidatorIds)",
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
        }
        for symbol, tokens in core_reconstruction_tokens.items():
            extracted = _top_level_operator_body(core_source, symbol)
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

    liveness_cfg = formal_dir / "liveness.cfg"
    if liveness_cfg.is_file():
        cfg_source = liveness_cfg.read_text(encoding="utf-8")
        if "INVARIANT ReceivedTimeoutVotePoolInvariant\n" not in cfg_source:
            errors.append(
                f"{liveness_cfg}: timeout-pool uniqueness must remain a TLC invariant"
            )
    return errors


def _chain_source_fidelity_errors(formal_dir: Path) -> list[str]:
    """Keep chain composition per-node and independent of the old global barrier."""

    chain_path = formal_dir / "SumeragiV2ChainEpoch.tla"
    proof_path = formal_dir / "SumeragiV2ChainEpochProofs.tla"
    refinement_path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    errors: list[str] = []

    if chain_path.is_file():
        source = strip_tla_comments(chain_path.read_text(encoding="utf-8"))
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
            scheduler_fields = (
                "asyncNow",
                "asyncCommandQueues",
                "asyncNextCommandClass",
                "asyncFifoOwed",
                "asyncTimeoutEmitted",
                "asyncRunnerPhase",
                "asyncRunnerBudget",
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
        }
        for symbol, expected_body in verification_helpers.items():
            extracted = _top_level_operator_body(raw_source, symbol)
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
                if proof_normalized != expected_proof_mapping:
                    errors.append(
                        f"{refinement_path}:{proof_line}: "
                        "VerificationAsyncProof must use the exact IndexedAsync "
                        "Core/scheduler tuple substitution through the "
                        "VerificationCore and VerificationScheduler mappings"
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
                "Len(indexedAsyncState[initialContext][1]) = 46",
                "Len(indexedAsyncState[initialContext][2]) = 31",
            )
            missing = [token for token in required if token not in normalized]
            if missing:
                errors.append(
                    f"{refinement_path}:{line}: IndexedAsyncStateShape has stale "
                    f"Core/scheduler tuple arity {missing}"
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
            if "UNCHANGED IndexedScheduler(initialContext, 23)" not in " ".join(
                body.split()
            ):
                errors.append(
                    f"{refinement_path}:{line}: indexed non-runner frame must "
                    "preserve scheduler slot 23 (asyncNodeServiceDeadlines)"
                )
    return errors


def _retired_path_present(path: Path) -> bool:
    """Treat an empty, untracked legacy directory as absent."""

    if path.is_dir() and not path.is_symlink():
        return any(path.iterdir())
    return path.exists()


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
    errors.extend(_chain_source_fidelity_errors(formal_dir))
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
