#!/usr/bin/env python3
"""Fail closed if pure command readiness drifts from the 13-arm executor."""

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
    # Top-level declarations are often separated by explanatory TLA comments.
    # They are not part of an operator's semantic body, even when the next
    # declaration header follows the comment rather than preceding it.
    without_comments = re.sub(r"\(\*.*?\*\)", " ", text, flags=re.DOTALL)
    return " ".join(without_comments.split())


def declaration(
    source: str, symbol: str, *, theorem: bool = False
) -> tuple[str, str]:
    prefix = (
        r"(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)\s+" if theorem else ""
    )
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
    ends = [len(source)]
    if following is not None:
        ends.append(following.start())
    if footer is not None:
        ends.append(match.end() + footer.start())
    body = source[match.end() : min(ends)]
    parts = re.split(r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b", body, maxsplit=1)
    return normalized(parts[0]), normalized(parts[1] if len(parts) == 2 else "")


ARMS = (
    "ExecuteRegularCommand",
    "ExecuteDecisionFetch",
    "ExecuteSignProposal",
    "ExecuteSignVote",
    "ExecuteFormPrepareQC",
    "ExecuteSignTimeout",
    "ExecutePersistInstall",
    "ExecutePersistDecision",
    "ExecuteRequestCertifiedBody",
    "ExecuteApply",
    "ExecuteCoreDelivery",
    "ExecuteChunkDelivery",
    "ExecuteRejectAuthenticatedJunk",
)

REGULAR_HELPER_MODULE = "SumeragiV2RegularCommandExecutionReadyProofs"
REGULAR_FRAMED_HELPER_MODULE = "SumeragiV2RegularCommandFramedReadyProofs"
NON_REGULAR_HELPER_MODULE = "SumeragiV2NonRegularCommandExecutionReadyProofs"

COMPOSED_ARM_HELPERS = {
    "ExecuteRegularCommand": (
        REGULAR_HELPER_MODULE,
        (
            "RegularCoreReadyDecomposesIntoLeaves",
            "RegularLeafReadyIffEnabled",
            "RegularExecutionDecomposesIntoLeaves",
            "ENABLEDaxioms",
        ),
    ),
    "ExecuteDecisionFetch": (NON_REGULAR_HELPER_MODULE, ()),
    "ExecuteSignProposal": (NON_REGULAR_HELPER_MODULE, ()),
    "ExecuteSignVote": (NON_REGULAR_HELPER_MODULE, ()),
    "ExecuteSignTimeout": (NON_REGULAR_HELPER_MODULE, ()),
    "ExecutePersistInstall": (NON_REGULAR_HELPER_MODULE, ()),
    "ExecutePersistDecision": (NON_REGULAR_HELPER_MODULE, ()),
    "ExecuteRequestCertifiedBody": (NON_REGULAR_HELPER_MODULE, ()),
    "ExecuteApply": (NON_REGULAR_HELPER_MODULE, ()),
}

READY_BODIES = {
    "ExecuteRegularCommand": normalized(
        r"""RegularCoreCommandReady(command)"""
    ),
    "ExecuteDecisionFetch": normalized(
        r"""CertifiedRecoveryFetchFrontier(command)"""
    ),
    "ExecuteSignProposal": normalized(
        r"""
/\ command.kind = "SignProposal"
/\ \E request \in signProposals:
     LET controlItems == ProposalOutbox(request)
     IN /\ CommandMatches(command, request.node, request.proposal.view,
                           request.proposal.subject)
        /\ CompleteProposalSignatureReady(request)
        /\ controlItems \subseteq
             {item \in AsyncNetworkItems:
                item.kind \in AsyncControlKinds}
"""
    ),
    "ExecuteSignVote": normalized(
        r"""
/\ command.kind = "SignVote"
/\ \E request \in signVotes:
     /\ CommandMatches(command, request.node, request.vote.view,
                       request.vote.subject)
     /\ CompleteVoteSignatureReady(request)
     /\ VoteOutbox(request) \subseteq
          {item \in AsyncNetworkItems:
             item.kind \in AsyncControlKinds}
"""
    ),
    "ExecuteFormPrepareQC": normalized(
        r"""
LET signers == ProjectedVoteSignersAt(
                   command.node, command.view, "Prepare", command.subject)
    qc == QC(context, command.view, "Prepare", command.subject, signers)
    items == QcOutbox(command.node, qc)
IN /\ command.kind = "FormPrepareQC"
   /\ FormPrepareQCReady(command.node, command.view, command.subject)
   /\ items \subseteq
        {item \in AsyncNetworkItems:
           item.kind \in AsyncControlKinds}
"""
    ),
    "ExecuteSignTimeout": normalized(
        r"""
/\ command.kind = "SignTimeout"
/\ \E request \in signTimeouts:
     /\ CommandMatches(command, request.node, request.vote.view,
                       request.vote.highSubject)
     /\ CompleteTimeoutSignatureReady(request)
     /\ TimeoutOutbox(request) \subseteq
          {item \in AsyncNetworkItems:
             item.kind \in AsyncControlKinds}
"""
    ),
    "ExecutePersistInstall": normalized(
        r"""
/\ command.kind = "PersistInstallTC"
/\ \E request \in pendingInstallTC:
     /\ command.node = request.node
     /\ command.view = request.tc.view
     /\ InstallTcEvidenceMatches(command, request.tc)
     /\ PersistInstallTCReady(request)
"""
    ),
    "ExecutePersistDecision": normalized(
        r"""
/\ command.kind = "PersistDecision"
/\ \E request \in pendingDecision:
     /\ CommandMatches(command, request.node, request.qc.view,
                       request.qc.subject)
     /\ PersistDecisionReady(request)
"""
    ),
    "ExecuteRequestCertifiedBody": normalized(
        r"""
/\ command.kind = "RequestCertifiedBody"
/\ ~BodyHeldBy(durableBodies, command.node, context, command.view,
                command.subject)
/\ \E qc \in DecisionQcValues \cup prepareQCs:
     /\ CommandMatches(command, command.node, qc.view, qc.subject)
     /\ command.evidence = qc
     /\ CertifiedBodyRecoveryAuthority(command.node, qc)
     /\ \A item \in CertifiedRequestOutbox(command.node, qc):
          item.kind = "CertifiedRequest"
"""
    ),
    "ExecuteApply": normalized(
        r"""
/\ command.kind = "Apply"
/\ \E qc \in DecisionQcValues:
     /\ CommandMatches(command, command.node, qc.view, qc.subject)
     /\ ApplyDecisionReady(command.node, qc)
"""
    ),
    "ExecuteCoreDelivery": normalized(
        r"""
LET item == command.item
IN /\ item \in asyncSentItems
   /\ AsyncControlServiceOccurrenceIsCurrentOwner(item)
   /\ command.node = item.envelope.recipient
   /\ \/ /\ command.kind = "DeliverProposal"
          /\ item.kind = "Proposal"
          /\ DeliverProposalReady(item.envelope)
      \/ /\ command.kind = "DeliverVote"
          /\ item.kind \in {"PrepareVote", "CommitVote"}
          /\ DeliverVoteReady(item.envelope)
      \/ /\ command.kind = "DeliverQC"
          /\ item.kind \in {"PrepareQC", "CommitQC"}
          /\ DeliverQCReady(item.envelope)
      \/ /\ command.kind = "DeliverTimeout"
          /\ item.kind = "TimeoutVote"
          /\ DeliverTimeoutReady(item.envelope)
      \/ /\ command.kind = "DeliverTC"
          /\ item.kind = "TimeoutCertificate"
          /\ DeliverTCReady(item.envelope)
"""
    ),
    "ExecuteChunkDelivery": normalized(
        r"""
LET item == command.item
IN /\ command.kind = "DeliverChunk"
   /\ item \in asyncSentItems
   /\ item.kind = "Chunk"
   /\ item.envelope.recipient = command.node
   /\ item.envelope.chunk \in AsyncChunks
"""
    ),
    "ExecuteRejectAuthenticatedJunk": normalized(
        r"""
LET item == command.item
IN /\ \/ /\ command.kind = "RejectNormal"
           /\ item.kind = "NormalJunk"
      \/ /\ command.kind = "RejectProgress"
           /\ item.kind = "ProgressJunk"
   /\ item \in asyncSentItems
   /\ item.envelope.recipient = command.node
"""
    ),
}


def validate(network_path: Path, proof_path: Path) -> list[str]:
    errors: list[str] = []
    try:
        network = network_path.read_text(encoding="utf-8")
        proof = proof_path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        return [f"cannot read command-readiness source: {error}"]

    helper_sources: dict[str, str] = {}
    for module in (
        REGULAR_HELPER_MODULE,
        REGULAR_FRAMED_HELPER_MODULE,
        NON_REGULAR_HELPER_MODULE,
    ):
        helper_path = proof_path.parent / f"{module}.tla"
        try:
            helper_sources[module] = helper_path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            errors.append(f"cannot read {module} source: {error}")

    for module in (REGULAR_HELPER_MODULE, NON_REGULAR_HELPER_MODULE):
        if module not in proof:
            errors.append(
                f"{proof_path}: exact command-readiness proof must import "
                f"{module}"
            )
    regular_helper = helper_sources.get(REGULAR_HELPER_MODULE, "")
    if (
        regular_helper
        and REGULAR_FRAMED_HELPER_MODULE not in regular_helper
    ):
        errors.append(
            f"{proof_path.parent}: regular command-readiness composition must "
            f"import {REGULAR_FRAMED_HELPER_MODULE}"
        )

    if "CommandExecutionEnabled" in network:
        errors.append(
            f"{network_path}: retired nested CommandExecutionEnabled operator remains"
        )

    expected_union = normalized(
        "\\E selectedCommand \\in {command}:\n"
        + "\n".join(
            ("  \\/ " if index == 0 else "  \\/ ")
            + f"{arm}Ready(selectedCommand)"
            for index, arm in enumerate(ARMS)
        )
    )
    try:
        ready_union, _ = declaration(network, "CommandExecutionReady")
    except ValueError as error:
        errors.append(f"{network_path}: {error}")
    else:
        if ready_union != expected_union:
            errors.append(
                f"{network_path}: CommandExecutionReady must retain the exact "
                f"13-arm canonical order {expected_union!r}; found {ready_union!r}"
            )

    expected_execute = normalized(
        "\n".join(
            ("\\/ " if index == 0 else "\\/ ") + f"{arm}(command)"
            for index, arm in enumerate(ARMS)
        )
    )
    try:
        execute_union, _ = declaration(network, "ExecuteCommand")
    except ValueError as error:
        errors.append(f"{network_path}: {error}")
    else:
        if execute_union != expected_execute:
            errors.append(
                f"{network_path}: ExecuteCommand must retain the matching exact "
                "13-arm canonical order"
            )

    for arm in ARMS:
        symbol = f"{arm}Ready"
        try:
            body, _ = declaration(network, symbol)
        except ValueError as error:
            errors.append(f"{network_path}: {error}")
            continue
        if body in {"TRUE", "FALSE"} or "ENABLED" in body:
            errors.append(
                f"{network_path}: {symbol} must be a non-tautological pure guard"
            )
        if body != READY_BODIES[arm]:
            errors.append(
                f"{network_path}: {symbol} must retain its exact normalized "
                f"production guard body {READY_BODIES[arm]!r}; found {body!r}"
            )

        theorem = f"{arm}ReadyIffEnabled"
        try:
            statement, arm_proof = declaration(proof, theorem, theorem=True)
        except ValueError as error:
            errors.append(f"{proof_path}: {error}")
            continue
        expected_statement = normalized(
            f"\\A command: {symbol}(command) "
            f"<=> ENABLED {arm}(command)"
        )
        if statement != expected_statement:
            errors.append(
                f"{proof_path}: {theorem} must retain the exact bidirectional "
                f"arm equivalence {expected_statement!r}; found {statement!r}"
            )
        if arm in COMPOSED_ARM_HELPERS:
            module, composed_dependencies = COMPOSED_ARM_HELPERS[arm]
            composed_theorem = f"{theorem}Composed"
            if arm_proof != composed_theorem:
                errors.append(
                    f"{proof_path}: {theorem} must be the exact source-fidelity "
                    f"alias of {composed_theorem}"
                )
            helper = helper_sources.get(module)
            if helper is None:
                continue
            try:
                composed_statement, composed_proof = declaration(
                    helper, composed_theorem, theorem=True
                )
            except ValueError as error:
                errors.append(f"{module}.tla: {error}")
                continue
            if composed_statement != expected_statement:
                errors.append(
                    f"{module}.tla: {composed_theorem} must retain the exact "
                    f"arm equivalence {expected_statement!r}"
                )
            if arm == "ExecuteRegularCommand":
                if composed_proof in {"", "TRUE", "FALSE"} or any(
                    dependency not in composed_proof
                    for dependency in composed_dependencies
                ):
                    errors.append(
                        f"{module}.tla: {composed_theorem} must retain the "
                        "leaf decomposition and modal transfer dependencies"
                    )
                continue

            ready_to_enabled = f"{arm}ReadyImpliesEnabled"
            enabled_to_ready = f"{arm}EnabledImpliesReady"
            if composed_proof != (
                f"{ready_to_enabled}, {enabled_to_ready}"
            ):
                errors.append(
                    f"{module}.tla: {composed_theorem} must compose the exact "
                    "two directional production proofs"
                )
            directional_specs = (
                (
                    ready_to_enabled,
                    normalized(
                        f"\\A command: {symbol}(command) "
                        f"=> ENABLED {arm}(command)"
                    ),
                    ("ReadyIffEnabled", "ENABLEDaxioms"),
                ),
                (
                    enabled_to_ready,
                    normalized(
                        f"\\A command: ENABLED {arm}(command) "
                        f"=> {symbol}(command)"
                    ),
                    ("ImpliesReadyProjection", "ProjectionIffReady",
                     "ENABLEDaxioms"),
                ),
            )
            for direction, direction_statement, dependencies in (
                directional_specs
            ):
                try:
                    actual_statement, direction_proof = declaration(
                        helper, direction, theorem=True
                    )
                except ValueError as error:
                    errors.append(f"{module}.tla: {error}")
                    continue
                if actual_statement != direction_statement:
                    errors.append(
                        f"{module}.tla: {direction} must retain its exact "
                        "directional readiness statement"
                    )
                if direction_proof in {"", "TRUE", "FALSE"} or any(
                    dependency not in direction_proof
                    for dependency in dependencies
                ):
                    errors.append(
                        f"{module}.tla: {direction} must retain its "
                        "source-connected witness/projection dependencies"
                    )
        else:
            required_dependencies = (
                "ExpandENABLED",
                symbol,
                arm,
                "vars",
            )
            if arm_proof in {"", "TRUE", "FALSE"} or any(
                dependency not in arm_proof
                for dependency in required_dependencies
            ):
                errors.append(
                    f"{proof_path}: {theorem} must retain its non-vacuous "
                    "action/readiness proof dependencies"
                )

    expected_dispatch = normalized(
        r"""
/\ AsyncCandidateTyped(command)
/\ CandidateConsumerCurrent(command)
/\ CommandExecutionReady(command)
/\ (NodeIdle(command.node)
      \/ command.class = "Completion"
      \/ LocalAssemblyBusyDispatchAllowed(command)
      \/ AsyncCandidateHasCertifiedFenceRoot(command))
"""
    )
    try:
        dispatch, _ = declaration(network, "CommandDispatchable")
    except ValueError as error:
        errors.append(f"{network_path}: {error}")
    else:
        if dispatch != expected_dispatch:
            errors.append(
                f"{network_path}: CommandDispatchable must call the exact pure "
                "CommandExecutionReady kernel once"
            )

    expected_names = normalized(
        '{"Regular", "DecisionFetch", "SignProposal", "SignVote", '
        '"FormPrepareQC", "SignTimeout", "PersistInstall", "PersistDecision", '
        '"RequestCertifiedBody", "Apply", "CoreDelivery", "ChunkDelivery", '
        '"RejectAuthenticatedJunk"}'
    )
    try:
        names, _ = declaration(proof, "CommandExecutionReadyArmNames")
    except ValueError as error:
        errors.append(f"{proof_path}: {error}")
    else:
        if names != expected_names:
            errors.append(
                f"{proof_path}: CommandExecutionReadyArmNames must retain the "
                "exact 13-member domain"
            )

    try:
        domain_statement, domain_proof = declaration(
            proof,
            "CommandExecutionReadyArmDomainHasExactlyThirteenMembers",
            theorem=True,
        )
    except ValueError as error:
        errors.append(f"{proof_path}: {error}")
    else:
        if domain_statement != (
            "Cardinality(CommandExecutionReadyArmNames) = 13"
        ):
            errors.append(
                f"{proof_path}: arm-domain theorem must retain exact "
                "cardinality 13"
            )
        cardinality_dependencies = (
            "FS_Singleton",
            "FS_AddElement",
            "CommandExecutionReadyArmNames",
        )
        labels = tuple(
            arm.removeprefix("Execute").removesuffix("Command")
            for arm in ARMS
        )
        if (
            domain_proof in {"", "TRUE", "FALSE"}
            or any(
                dependency not in domain_proof
                for dependency in cardinality_dependencies
            )
            or domain_proof.count("FS_Singleton") != 1
            or domain_proof.count("FS_AddElement") != 12
            or any(f'"{label}"' not in domain_proof for label in labels)
        ):
            errors.append(
                f"{proof_path}: arm-domain theorem must retain its exact "
                "13-member finite-set induction"
            )

    theorem = "CommandExecutionReadyExactlyCharacterizesEnabledAction"
    try:
        statement, theorem_proof = declaration(proof, theorem, theorem=True)
    except ValueError as error:
        errors.append(f"{proof_path}: {error}")
    else:
        expected_statement = (
            "\\A command: CommandExecutionReady(command) "
            "<=> ENABLED ExecuteCommand(command)"
        )
        if statement != expected_statement:
            errors.append(
                f"{proof_path}: {theorem} must state only the bidirectional "
                "pure-readiness/ENABLED equivalence"
            )
        expected_lemmas = tuple(f"{arm}ReadyIffEnabled" for arm in ARMS)
        positions = [theorem_proof.find(lemma) for lemma in expected_lemmas]
        if any(position < 0 for position in positions) or positions != sorted(positions):
            errors.append(
                f"{proof_path}: {theorem} must consume all 13 arm equivalence "
                "lemmas once in canonical order"
            )
        if theorem_proof.count("ReadyIffEnabled") != 13:
            errors.append(
                f"{proof_path}: {theorem} must consume exactly 13 arm lemmas"
            )

    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("network", type=Path)
    parser.add_argument("proof", type=Path)
    args = parser.parse_args()
    errors = validate(args.network, args.proof)
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    print("CommandExecutionReady source contract is exact")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
