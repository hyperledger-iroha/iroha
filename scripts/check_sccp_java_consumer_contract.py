#!/usr/bin/env python3
"""Check SCCP Java consumer execution and closed Kotlin API from compiled metadata.

No JVM reflection, runtime loading or compatibility API is introduced. This
source/bytecode/report contract does not replace signed candidate provenance or
physical Android qualification.
"""
from __future__ import annotations

import argparse
import hashlib
import io
import json
import math
from pathlib import Path
import sys
import xml.etree.ElementTree as ET
import zipfile

from jvm_classfile import ACC_PUBLIC, MAX_CLASS_BYTES, ClassFileError, parse_class
from sccp_release_common import SccpReleaseError, read_direct_file, read_relative_file

EXPECTED_GROUPS = {'org.hyperledger.iroha.sdk.sccp.SccpV1JavaConsumerTest': ('replayForestMatchesLocalFinalV1Vector', 'replayOperationPrincipalAndTonDirectionsAreExact', 'soraReplayPrincipalRequiresExactCanonicalAccountIdPayload', 'closedInventoryReservesRetiredTagsAndAliases', 'tonMainnetBindsCanonicalZeroState', 'allSharedMainnetTransferVectorsMatchRust', 'governedHashRotationPreservesReplayIdentityButChangesCommitment', 'payloadDecoderRejectsRetiredAndNoncanonicalForms', 'transferRejectsRetiredDomainsCodecsAndInvalidWidths', 'canonicalTextAcceptsExactI105AndRejectsUnicodeSubstitutions', 'contextAndCommitmentRejectZeroOrAliasedHashRoles', 'commitmentDecoderRejectsTamperingCollisionsAndTrailingBytes', 'payloadAndContextDefensivelyCopyBuffers'), 'org.hyperledger.iroha.sdk.client.SccpClientExactJavaConsumerTest': ('submitDtosExposeOnlyClosedArtifactFields', 'binaryProofRequestAcceptsOnlyTheTwoConcreteCurveTypes', 'transactionCodecPreservesExactTairaSponsorAcrossControllerOnlyWireIdentity', 'submitAuthorityRequiresExactTairaDiscriminant', 'submitPreflightRejectsRetiredOverridesAndSecrets', 'artifactValidationRejectsAliasesCorruptionAndZeroSchema', 'capabilitiesAreExactAndContainNoRetiredDiscoverySurface', 'registryValidatesSemanticPolicyAndExactFamilies', 'registryValidatesExactTonDeploymentAndRoleSeparation', 'registryRejectsMalformedAnchorHistories', 'registryRequiresExactRetiredRouteInboundFinalityCutoff', 'registryCountsOnlyNonRetiredRoutes', 'verifyingKeysAllowZeroCoordinatesButRejectInfinity', 'routeConfigurationIsNetworkExactAndPolicyBound', 'bundleAndProofRequestRejectRetiredAndAliasedRoles', 'tonProofRequestBindsExactBlsSignalsAndProfile', 'recentMessagesRequireExactLinksAndUniqueIds', 'detachedSigningResponseRejectsCrossFamilyLabels')}
API_OWNERS = {
    "org/hyperledger/iroha/sdk/client/IrohaClient": "IrohaClient.kt",
    "org/hyperledger/iroha/sdk/client/HttpClientTransport": "HttpClientTransport.kt",
}
RETIRED_WRITES = ("submitSccpDestinationProof", "submitSccpNativeMessage")
MAX_REPORT_BYTES = 16 * 1024 * 1024


class ContractError(ValueError):
    """A required compiled owner or executed assertion group is missing or invalid."""


def read_exact_file(path: Path, limit: int) -> bytes:
    """Reuse the corridor's bounded stable regular-file ownership check."""
    return read_direct_file(path, label="SCCP consumer contract input", maximum=limit)


RELEASE8_TERMINALS = ("java/lang/Object", "java/lang/AutoCloseable")


def load_release8_terminals(jdk_home: Path) -> tuple[dict[str, object], list[dict[str, str]]]:
    """Inspect these exact JDK 8 API terminals from the selected compiler's ct.sym."""
    path = jdk_home / "lib/ct.sym"
    raw = read_exact_file(path, 64 * 1024 * 1024)
    records = [{"path": str(path), "sha256": hashlib.sha256(raw).hexdigest()}]
    classes = {}
    with zipfile.ZipFile(io.BytesIO(raw)) as archive:
        entries = archive.infolist()
        if len(entries) > 100_000:
            raise ContractError("JDK symbol inventory exceeds its bound")
        for name in RELEASE8_TERMINALS:
            suffix = "/java.base/" + name + ".sig"
            matches = [entry for entry in entries if entry.filename.endswith(suffix) and "8" in entry.filename.split("/", 1)[0]]
            if len(matches) != 1 or matches[0].file_size > MAX_CLASS_BYTES:
                raise ContractError(f"missing or ambiguous JDK 8 terminal: {name}")
            entry = matches[0]
            body = archive.read(entry)
            owner = parse_class(body)
            if owner.name != name or owner.interfaces or owner.super_name != (None if name == "java/lang/Object" else "java/lang/Object"):
                raise ContractError(f"unexpected JDK 8 terminal hierarchy: {name}")
            if any(method.name in RETIRED_WRITES and method.flags & ACC_PUBLIC for method in owner.methods):
                raise ContractError(f"retired SCCP method in JDK 8 terminal: {name}")
            classes[name] = owner
            records.append({"path": str(path) + "!/" + entry.filename, "sha256": hashlib.sha256(body).hexdigest()})
    return classes, records


def validate_api(classes: dict[str, object]) -> None:
    """Retain both getMethods negatives, including every transitive public parent."""
    for name, source in API_OWNERS.items():
        owner = classes.get(name)
        if owner is None or owner.name != name or owner.major != 52 or owner.source_file != source:
            raise ContractError(f"required Kotlin JDK 8 API owner is missing: {name}")
        complete = set()
        active = set()
        def visit(current: str) -> None:
            if current in complete:
                return
            if len(active) >= 256:
                raise ContractError("compiled API inheritance exceeds its depth bound")
            if current in active:
                raise ContractError("cyclic compiled API inheritance")
            candidate = classes.get(current)
            if candidate is None or candidate.name != current:
                raise ContractError(f"required API parent is missing or unknown: {current}")
            if current not in RELEASE8_TERMINALS and not current.startswith("org/hyperledger/iroha/sdk/"):
                raise ContractError(f"unreviewed external API parent: {current}")
            if current not in RELEASE8_TERMINALS and candidate.major != 52:
                raise ContractError(f"API ancestor is not JDK 8 bytecode: {current}")
            if any(method.name in RETIRED_WRITES and method.flags & ACC_PUBLIC for method in candidate.methods):
                raise ContractError(f"unbound SCCP public write method was restored: {name} via {current}")
            active.add(current)
            for parent in ((candidate.super_name,) if candidate.super_name is not None else ()) + candidate.interfaces:
                visit(parent)
            active.remove(current)
            complete.add(current)
        visit(name)


def validate_test_owner(owner, name: str) -> None:
    """Require all original Java assertion entrypoints on their one compiled owner."""
    expected = EXPECTED_GROUPS[name]
    if owner.name != name.replace(".", "/") or owner.major != 52 or owner.source_file != name.rsplit(".", 1)[1] + ".java":
        raise ContractError(f"wrong Java consumer bytecode owner: {name}")
    for group in expected:
        matches = [method for method in owner.methods if method.name == group]
        if len(matches) != 1 or matches[0].descriptor != "()V" or matches[0].static or not matches[0].flags & ACC_PUBLIC:
            raise ContractError(f"required Java assertion entrypoint missing: {name}.{group}")


def validate_report(raw: bytes, name: str) -> None:
    """Reject skipped, failed, repeated, absent or zero-selected Java groups."""
    if b"<!DOCTYPE" in raw or b"<!ENTITY" in raw:
        raise ContractError("report must not contain a DTD or entity declaration")
    try:
        root = ET.fromstring(raw)
    except ET.ParseError as error:
        raise ContractError("invalid JUnit report") from error
    expected = EXPECTED_GROUPS[name]
    if root.tag != "testsuite" or root.attrib.get("name") != name:
        raise ContractError("wrong JUnit test suite")
    if root.attrib.get("tests") != str(len(expected)):
        raise ContractError("JUnit test count does not equal the complete Java suite")
    if any(root.attrib.get(field) != "0" for field in ("failures", "errors", "skipped")):
        raise ContractError("JUnit Java suite failed, skipped or omitted its result counts")
    cases = root.findall("testcase")
    found = []
    for case in cases:
        if case.attrib.get("classname") != name or any(case.find(tag) is not None for tag in ("failure", "error", "skipped")):
            raise ContractError("JUnit Java case did not execute successfully")
        case_name = case.attrib.get("name", "")
        if not case_name.endswith("()"):
            raise ContractError("JUnit Java case must use its exact method display name")
        found.append(case_name[:-2])
        try:
            duration = float(case.attrib["time"])
        except (KeyError, ValueError) as error:
            raise ContractError("JUnit Java case lacks a valid duration") from error
        if not math.isfinite(duration) or duration < 0:
            raise ContractError("invalid JUnit Java case duration")
    if len(found) != len(expected) or set(found) != set(expected):
        raise ContractError("JUnit Java groups are missing, duplicated or substituted")


def audit(main_classes: Path, test_classes: Path, reports: Path, jdk_home: Path) -> dict[str, object]:
    """Bind native Kotlin API owners, Java 8 consumers and all executed groups."""
    classes, records = load_release8_terminals(jdk_home)
    def capture(root: Path, relative: str, bound: int) -> bytes:
        raw = read_relative_file(root, relative, label="SCCP compiled consumer input", maximum=bound)
        records.append({"path": str(root / relative), "sha256": hashlib.sha256(raw).hexdigest()})
        return raw
    pending = list(API_OWNERS)
    while pending:
        name = pending.pop()
        if name in classes:
            continue
        if not name.startswith("org/hyperledger/iroha/sdk/") or len(classes) >= 20_000:
            raise ContractError(f"unknown or excessive API ancestry: {name}")
        owner = parse_class(capture(main_classes, name + ".class", MAX_CLASS_BYTES))
        if owner.name != name:
            raise ContractError("compiled API class identity mismatch")
        classes[name] = owner
        pending.extend(((owner.super_name,) if owner.super_name is not None else ()) + owner.interfaces)
    validate_api(classes)
    for name in EXPECTED_GROUPS:
        relative = name.replace(".", "/") + ".class"
        validate_test_owner(parse_class(capture(test_classes, relative, MAX_CLASS_BYTES)), name)
        validate_report(capture(reports, "TEST-" + name + ".xml", MAX_REPORT_BYTES), name)
    return {"schema": "iroha.sccp.java_consumer_contract.v1", "status": "passed", "assertion_groups": sum(map(len, EXPECTED_GROUPS.values())), "inputs": records}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jdk-home", type=Path, required=True)
    parser.add_argument("--main-classes", type=Path, required=True)
    parser.add_argument("--test-classes", type=Path, required=True)
    parser.add_argument("--reports", type=Path, required=True)
    args = parser.parse_args()
    try:
        result = audit(args.main_classes, args.test_classes, args.reports, args.jdk_home)
    except (OSError, SccpReleaseError, ClassFileError, ContractError, zipfile.BadZipFile) as error:
        print(f"SCCP Java consumer contract rejected: {error}", file=sys.stderr)
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
