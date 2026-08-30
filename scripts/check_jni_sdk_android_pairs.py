#!/usr/bin/env python3
"""Freeze the paired Kotlin/JVM and Java/Android JNI wrapper inventory."""

from __future__ import annotations

import hashlib
import re
import sys
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
JNI_SOURCE = REPO_ROOT / "crates/connect_norito_bridge/src/platform_jni/part_3.rs"
SDK_PREFIX = "Java_org_hyperledger_iroha_sdk_"
ANDROID_PREFIX = "Java_org_hyperledger_iroha_android_"
PAIR_MACRO_NAME = "jni_sdk_android_pairs"
FORWARDER_MACRO_NAME = "kagemusha_sdk_android_forwarders"
EXPECTED_PAIR_MACRO_DIGEST = "75234f8e3dfcdaa54347f628fd7fb7118de18003baed0e3c37750cd283db2468"
EXPECTED_FORWARDER_MACRO_DIGEST = "1f724823e381a3d0fea9d26b76a073b1d8224becccda062255f11978f909d8ad"
EXPECTED_ABI_DIGEST = "cab490ffb44446f846e7b063faf34fa81068a232c34d44f0dc3c56e94ad85e63"
EXPECTED_ATTRIBUTE_DIGEST = (
    "2d3d049cb33bda4d4a3d6da9afe3cffa110aec2e7eebdc2473f5276638527112"
)

EXPECTED_METHODS = {
    "crypto_NativeSignerBridge": (
        "nativePublicKeyFromPrivate",
        "nativeBridgeAbiVersion",
        "nativeSignerContractRevision",
        "nativeKeypairFromSeed",
        "nativeSignDetached",
        "nativeVerifyDetached",
        "nativeEncodeRegisterZkAssetSignedTransaction",
    ),
    "privacy_PrivacyNativeBridge": (
        "nativeBridgeAbiVersion",
        "nativeCompiledProfileCatalog",
        "nativeValidateCompiledProfileCatalog",
        "nativeValidateExact12CapabilityManifest",
        "nativeInspectExact12CapabilityManifest",
        "nativeRequireExact12CapabilityTuple",
        "nativeValidateExact12SubmitProofConstruction",
        "nativeInspectSignedExact12ActionV1",
        "nativeExact12FixtureBundle",
        "nativeValidateExact12FixtureBundle",
    ),
    "sorafs_SorafsReferenceValidators": (
        "nativeBridgeAbiVersion",
        "nativeHasGovernanceDagSymbols",
        "nativeHasGovernanceLogNodeSymbols",
        "nativeHasFixtureBundleSymbols",
        "nativeHasAppealFinanceSymbols",
        "nativeValidateOrderbookPayloadJson",
        "nativeValidatePopPayloadJson",
        "nativeValidateHedgingPayloadJson",
        "nativeValidateAppealFinanceCancelAssetLockJson",
        "nativeValidateFixtureBundleJson",
        "nativeValidateGovernanceLogNodeJson",
        "nativeValidateGovernanceDagBlockJson",
        "nativeValidateGovernanceDagHeadChainJson",
        "nativeSignOrderbookPayload",
        "nativeDeriveOrderbookOrderId",
        "nativeBuildSignedOrderbookOrderRequest",
        "nativeBuildSignedOrderbookOrderCancel",
        "nativeBuildSignedOrderbookSettlementReceipt",
        "nativeValidatePdpPayloadJson",
        "nativeValidatePdpCommitmentChallengeJson",
        "nativeValidatePdpChallengeProofJson",
        "nativeValidatePdpBundleJson",
    ),
    "client_AuthenticatedTransactionDetailsNativeBridge": (
        "nativeBridgeAbiVersion",
        "nativePrepareExactRejectedTransactionQueryV1",
        "nativeFinalizeExactRejectedTransactionQueryV1",
        "nativeProjectExactCommittedRejectionV1",
        "nativeProjectExactCommittedTransactionResultV1",
        "nativePrepareExactTransactionQueryV2",
        "nativeFinalizeExactTransactionQueryV2",
        "nativeProjectExactCommittedRejectionV2",
        "nativeProjectExactKagemushaCommittedRejectionV2",
        "nativeProjectExactCommittedTransactionResultV2",
        "nativeBindFinalityProofPageV1",
        "nativeVerifyFinalityPageV1",
        "nativeProjectFinalizedKagemushaOutcomeV1",
        "nativeProjectExactOfflineDeviceRegistrationResultV1",
    ),
    "client_AuthenticatedPrivacyActionReceiptNativeBridge": (
        "nativeBridgeAbiVersion",
        "nativeProjectFinalizedPrivacyActionRejectionV1",
        "nativePreparePrivacyActionReceiptQueryV1",
        "nativeFinalizePrivacyActionReceiptQueryV1",
        "nativeProjectPrivacyActionReceiptV1",
    ),
    "client_AuthenticatedPrivacyStateQueryNativeBridge": (
        "nativeBridgeAbiVersion",
        "nativePreparePrivacyStateQueryV1",
        "nativeFinalizePrivacyStateQueryV1",
        "nativeProjectPrivacyStateQueryV1",
    ),
    "offline_KagemushaRecursiveSpendProver": (
        "nativeBridgeAbiVersion",
        "nativePastaCycleV4BackendAvailable",
        "nativeArtifactBeginV4",
        "nativeArtifactWriteV4",
        "nativeArtifactFinalizeV4",
        "nativeArtifactCancelV4",
        "nativeArtifactSetInstallV4",
        "nativeArtifactSetIsInstalledV4",
        "nativeInstalledManifestSha256V4",
        "nativeBuildArtifactBindingV4",
        "nativeArtifactSetUninstallV4",
        "nativeInitSpendV4",
        "nativeAppendSpendV4",
        "nativeVerifySpendV4",
        "nativeBuildRedeemV4",
        "nativePrepareRecipientRequestV2",
        "nativeCreateRecipientRequestV2",
        "nativeVerifyRecipientRequestV2",
        "nativeCreateRecipientLineageQueryV2",
        "nativeVerifyRecipientRegistrationLineageV2",
        "nativeCreateRecipientReceiveOfferV2",
        "nativeProjectRecipientReceiveOfferV2",
        "nativeVerifyRecipientReceiveOfferV2",
        "nativeBuildOutputMembershipFrontierV4",
        "nativeDeriveOutputMembershipPathsV4",
        "nativeValidateSpendableBranchV4",
        "nativeBuildOutputMembershipPathsV4",
        "nativeBuildInitRequestV4",
        "nativeProjectVerifiedTopUpFinalityV4",
        "nativeBuildTopUpProvenanceV4",
        "nativeValidateTopUpProvenanceV4",
        "nativeBuildAppendRequestV4",
        "nativeBuildVerifyRequestV4",
        "nativeBuildRedeemRequestV4",
        "nativeProjectPeerPaymentV4",
        "nativeEncodeOfflineDevicePolicyProofRequestV1",
        "nativeVerifyOfflineDevicePolicyProofV1",
        "nativeEncodeOfflineDeviceEligibilityRequestV1",
        "nativeVerifyOfflineDeviceEligibilityResponseV1",
        "nativeVerifyOfflineDeviceAttestationPolicyViewV1",
        "nativeProjectOfflineDeviceAttestationPolicyViewClaimsV1",
        "nativeVerifyOfflineDeviceEligibilityCredentialV1",
        "nativeVerifyOfflineDeviceEligibilityPeerCertificateV1",
        "nativePrepareEligibilityPaymentV1",
        "nativeEligibilityPaymentSigningBytesV1",
        "nativeFinalizeEligibilityPaymentV1",
        "nativeValidateEligibilityPaymentStaticV1",
        "nativeValidateEligibilityPaymentFirstDeliveryV1",
        "nativeProjectInitResultV4",
        "nativeProjectSplitResultV4",
        "nativeProjectVerifyResultV4",
        "nativeProjectRedeemBuildResultV4",
        "nativePrepareAcknowledgementV2",
        "nativeCreateAcknowledgementV2",
        "nativeVerifyAcknowledgementV2",
        "nativeProjectReadinessV4",
        "nativeProjectAuthenticatedArtifactSetV4",
        "nativeProjectActiveVerifierV2",
        "nativePrepareAuthorizationV2",
        "nativeFinalizeDrainOnlyRedemptionAuthorizationV1",
        "nativeBuildDrainOnlyRedeemInstructionV4",
        "nativeFinalizeHardwareAuthorizationV2",
        "nativeFinalizeIosAppAttestAuthorizationV2",
        "nativeFinalizeTopUpV4",
        "nativeFinalizeRedeemV4",
        "nativePrepareTopUpV4",
        "nativeProjectOperationReferenceV1",
        "nativeProjectOperationStatusV4",
        "nativeBranchClaimsConflictV2",
        "nativePrepareRedemptionChangeV4",
        "nativePreparePeerSplitChangeV4",
        "nativePrepareNoteOpeningV2",
        "nativeProjectRecipientRequestV2",
        "nativeValidateEligibilityPaymentFirstDeliveryFinalizedV1",
    ),
}
EXPECTED_SUFFIXES = tuple(
    f"{bridge}_{method}"
    for bridge, methods in EXPECTED_METHODS.items()
    for method in methods
)


class AuditError(ValueError):
    """Raised when the paired JNI source contract is no longer exact."""


@dataclass(frozen=True)
class AuditResult:
    """Summary of the authenticated paired-wrapper inventory."""

    pair_count: int
    abi_digest: str
    attribute_digest: str


@dataclass(frozen=True)
class _PairRecord:
    """One source-ordered paired JNI wrapper contract."""

    suffix: str
    abi_record: str
    attribute_record: str
    source_offset: int
    generated: bool


def _skip_quoted(source: str, index: int) -> int | None:
    """Return the first byte after a Rust string or character literal."""

    raw_start = index
    if source.startswith("br", index):
        raw_start += 2
    elif source.startswith("r", index):
        raw_start += 1
    else:
        raw_start = -1
    if raw_start >= 0:
        hashes = 0
        while raw_start + hashes < len(source) and source[raw_start + hashes] == "#":
            hashes += 1
        quote = raw_start + hashes
        if quote < len(source) and source[quote] == '"':
            terminator = '"' + "#" * hashes
            end = source.find(terminator, quote + 1)
            if end < 0:
                raise AuditError("unterminated raw string while scanning JNI source")
            return end + len(terminator)
    if source[index] == '"':
        cursor = index + 1
        while cursor < len(source):
            if source[cursor] == "\\":
                cursor += 2
            elif source[cursor] == '"':
                return cursor + 1
            else:
                cursor += 1
        raise AuditError("unterminated string while scanning JNI source")
    if source[index] == "'":
        if index + 1 < len(source) and source[index + 1] == "\\":
            cursor = index + 2
            while cursor < len(source):
                if source[cursor] == "\\":
                    cursor += 2
                elif source[cursor] == "'":
                    return cursor + 1
                else:
                    cursor += 1
            raise AuditError("unterminated character literal while scanning JNI source")
        if index + 2 < len(source) and source[index + 2] == "'":
            return index + 3
    return None


def _matching_brace(source: str, opening: int) -> int:
    """Find the closing brace while ignoring comments and quoted braces."""

    if opening >= len(source) or source[opening] != "{":
        raise AuditError("brace scanner did not start on an opening brace")
    depth = 0
    cursor = opening
    block_comment_depth = 0
    while cursor < len(source):
        if block_comment_depth:
            if source.startswith("/*", cursor):
                block_comment_depth += 1
                cursor += 2
            elif source.startswith("*/", cursor):
                block_comment_depth -= 1
                cursor += 2
            else:
                cursor += 1
            continue
        if source.startswith("//", cursor):
            newline = source.find("\n", cursor + 2)
            cursor = len(source) if newline < 0 else newline + 1
            continue
        if source.startswith("/*", cursor):
            block_comment_depth = 1
            cursor += 2
            continue
        quoted_end = _skip_quoted(source, cursor)
        if quoted_end is not None:
            cursor = quoted_end
            continue
        if source[cursor] == "{":
            depth += 1
        elif source[cursor] == "}":
            depth -= 1
            if depth == 0:
                return cursor
            if depth < 0:
                break
        cursor += 1
    raise AuditError("unbalanced braces in paired JNI source")


def _attribute_text(fragment: str, platform: str) -> str:
    """Validate and canonicalize the attribute lines before one wrapper."""

    attributes = []
    for raw_line in fragment.splitlines():
        line = raw_line.strip()
        if not line:
            continue
        if not (line.startswith("///") or (line.startswith("#[") and line.endswith("]"))):
            raise AuditError(f"unexpected {platform} wrapper preamble: {line}")
        if raw_line != line:
            raise AuditError(f"{platform} wrapper attributes must stay at item indentation")
        attributes.append(line + "\n")
    return "".join(attributes)


def _macro_definition_end(
    source: str,
    name: str,
    expected_digest: str,
    label: str,
    *,
    must_lead: bool = False,
) -> int:
    """Authenticate one macro definition and return its first trailing byte."""

    definition_start = source.find(f"macro_rules! {name}")
    if definition_start < 0 or (must_lead and definition_start != 0):
        qualifier = " and lead part_3.rs" if must_lead else ""
        raise AuditError(f"{label} macro definition must exist{qualifier}")
    opening = source.find("{", definition_start)
    if opening < 0:
        raise AuditError(f"{label} macro definition has no body")
    closing = _matching_brace(source, opening)
    macro_definition = source[definition_start : closing + 1]
    macro_digest = hashlib.sha256(" ".join(macro_definition.split()).encode()).hexdigest()
    if macro_digest != expected_digest:
        raise AuditError(
            f"{label} macro expansion contract changed: "
            f"expected {expected_digest}, found {macro_digest}"
        )
    return closing + 1


def _macro_invocations(
    source: str, name: str, search_start: int
) -> list[tuple[str, int, int]]:
    """Return every invocation body and its source boundaries."""

    pattern = re.compile(rf"\b{re.escape(name)}!\s*\{{")
    invocations = []
    cursor = search_start
    while match := pattern.search(source, cursor):
        opening = source.index("{", match.start(), match.end())
        closing = _matching_brace(source, opening)
        invocations.append((source[opening + 1 : closing], opening + 1, closing))
        cursor = closing + 1
    return invocations


def _explicit_pair_records(body: str, body_start: int) -> list[_PairRecord]:
    """Parse one explicit paired-wrapper macro invocation."""

    records = []
    cursor = 0
    while True:
        cursor += len(body[cursor:]) - len(body[cursor:].lstrip())
        if cursor == len(body):
            break
        record_start = cursor
        if not body.startswith("android:", cursor):
            raise AuditError(f"unexpected token before paired wrapper {len(records) + 1}")
        android_preamble_start = cursor + len("android:")
        android_match = re.search(
            rf"fn ({re.escape(ANDROID_PREFIX)}[A-Za-z0-9_]+)\(\);",
            body[android_preamble_start:],
        )
        if android_match is None:
            raise AuditError("paired wrapper is missing its full Android export identifier")
        android_start = android_preamble_start + android_match.start()
        android_end = android_preamble_start + android_match.end()
        android_name = android_match.group(1)
        android_attributes = _attribute_text(
            body[android_preamble_start:android_start], "Android"
        )
        if "#[unsafe(no_mangle)]" in android_attributes:
            raise AuditError("Android no_mangle must be supplied exactly once by the pair macro")

        sdk_label = re.match(r"\s*sdk:", body[android_end:])
        if sdk_label is None:
            raise AuditError(f"{android_name} is not followed by its SDK wrapper")
        sdk_preamble_start = android_end + sdk_label.end()
        sdk_item = body.find('pub unsafe extern "system" fn ', sdk_preamble_start)
        if sdk_item < 0:
            raise AuditError(f"{android_name} has no unsafe system SDK function item")
        sdk_attributes = _attribute_text(body[sdk_preamble_start:sdk_item], "SDK")
        if sdk_attributes.count("#[unsafe(no_mangle)]\n") != 1:
            raise AuditError("SDK wrapper must retain exactly one unsafe no_mangle attribute")
        sdk_match = re.match(
            rf'pub unsafe extern "system" fn ({re.escape(SDK_PREFIX)}[A-Za-z0-9_]+)\(',
            body[sdk_item:],
        )
        if sdk_match is None:
            raise AuditError(f"{android_name} has a malformed SDK function declaration")
        sdk_name = sdk_match.group(1)
        body_open = body.find("{", sdk_item + sdk_match.end())
        if body_open < 0:
            raise AuditError(f"{sdk_name} has no function body")
        body_close = _matching_brace(body, body_open)
        function_item = body[sdk_item : body_close + 1]

        sdk_suffix = sdk_name.removeprefix(SDK_PREFIX)
        android_suffix = android_name.removeprefix(ANDROID_PREFIX)
        if sdk_suffix != android_suffix:
            raise AuditError(
                f"paired export suffix mismatch: SDK {sdk_suffix}, Android {android_suffix}"
            )
        records.append(
            _PairRecord(
                suffix=sdk_suffix,
                abi_record=(
                    sdk_suffix
                    + "\0"
                    + function_item.replace(sdk_name, "__JNI_EXPORT__", 1)
                ),
                attribute_record=(
                    sdk_suffix
                    + "\0"
                    + sdk_attributes
                    + "\0"
                    + android_attributes
                    + "#[unsafe(no_mangle)]\n"
                ),
                source_offset=body_start + record_start,
                generated=False,
            )
        )
        cursor = body_close + 1
    return records


def _top_level_statements(body: str) -> list[tuple[str, int]]:
    """Split a macro body on top-level semicolons."""

    statements = []
    start = 0
    cursor = 0
    braces = parentheses = brackets = 0
    block_comment_depth = 0
    while cursor < len(body):
        if block_comment_depth:
            if body.startswith("/*", cursor):
                block_comment_depth += 1
                cursor += 2
            elif body.startswith("*/", cursor):
                block_comment_depth -= 1
                cursor += 2
            else:
                cursor += 1
            continue
        if body.startswith("//", cursor):
            newline = body.find("\n", cursor + 2)
            cursor = len(body) if newline < 0 else newline + 1
            continue
        if body.startswith("/*", cursor):
            block_comment_depth = 1
            cursor += 2
            continue
        quoted_end = _skip_quoted(body, cursor)
        if quoted_end is not None:
            cursor = quoted_end
            continue
        token = body[cursor]
        if token == "{":
            braces += 1
        elif token == "}":
            braces -= 1
        elif token == "(":
            parentheses += 1
        elif token == ")":
            parentheses -= 1
        elif token == "[":
            brackets += 1
        elif token == "]":
            brackets -= 1
        elif token == ";" and braces == parentheses == brackets == 0:
            statements.append((body[start : cursor + 1], start))
            start = cursor + 1
        if braces < 0 or parentheses < 0 or brackets < 0:
            raise AuditError("unbalanced delimiters in generated JNI pair inventory")
        cursor += 1
    if block_comment_depth or braces or parentheses or brackets:
        raise AuditError("unbalanced delimiters in generated JNI pair inventory")
    if body[start:].strip():
        raise AuditError("generated JNI pair inventory has an unterminated entry")
    return statements


def _forwarder_pair_records(body: str, body_start: int) -> list[_PairRecord]:
    """Parse the compact Kagemusha paired-forwarder inventory."""

    records = []
    for statement, statement_start in _top_level_statements(body):
        entry = statement.strip()
        cursor = 0
        attributes = []
        while True:
            cursor += len(entry[cursor:]) - len(entry[cursor:].lstrip())
            if not entry.startswith("#[", cursor):
                break
            closing = entry.find("]", cursor + 2)
            if closing < 0 or "\n" in entry[cursor : closing + 1]:
                raise AuditError("generated JNI attributes must be complete one-line items")
            attributes.append(entry[cursor : closing + 1])
            cursor = closing + 1
        method_match = re.match(r"(native[A-Za-z0-9_]+)\s*\{", entry[cursor:])
        if method_match is None:
            raise AuditError("generated JNI pair entry has a malformed method name")
        method = method_match.group(1)
        opening = cursor + method_match.end() - 1
        closing = _matching_brace(entry, opening)
        trailer = entry[closing + 1 :]
        if re.fullmatch(
            r"\s*->\s*(?:\(\)|[A-Za-z][A-Za-z0-9_]*)\s*=\s*"
            r"[A-Za-z][A-Za-z0-9_:]*(?:\s*,[\s\S]*)?;\s*",
            trailer,
        ) is None:
            raise AuditError(f"generated JNI pair entry has a malformed delegate: {method}")
        suffix = f"offline_KagemushaRecursiveSpendProver_{method}"
        attribute_text = "".join(attribute + "\n" for attribute in attributes)
        records.append(
            _PairRecord(
                suffix=suffix,
                abi_record=suffix + "\0" + entry,
                attribute_record=(
                    suffix + "\0" + attribute_text + "\0" + attribute_text
                ),
                source_offset=body_start + statement_start,
                generated=True,
            )
        )
    return records


def audit_source(source: str) -> AuditResult:
    """Validate the exact paired export, signature, body, and attribute inventory."""

    pair_definition_end = _macro_definition_end(
        source,
        PAIR_MACRO_NAME,
        EXPECTED_PAIR_MACRO_DIGEST,
        "paired JNI",
        must_lead=True,
    )
    forwarder_definition_end = _macro_definition_end(
        source,
        FORWARDER_MACRO_NAME,
        EXPECTED_FORWARDER_MACRO_DIGEST,
        "Kagemusha paired-forwarder",
    )
    pair_invocations = _macro_invocations(source, PAIR_MACRO_NAME, pair_definition_end)
    forwarder_invocations = _macro_invocations(
        source, FORWARDER_MACRO_NAME, forwarder_definition_end
    )
    if len(pair_invocations) != 3 or len(forwarder_invocations) != 1:
        raise AuditError(
            "paired JNI macro topology changed: expected three explicit blocks and "
            "one Kagemusha forwarder block"
        )
    records = []
    for body, body_start, _body_end in pair_invocations:
        records.extend(_explicit_pair_records(body, body_start))
    for body, body_start, _body_end in forwarder_invocations:
        records.extend(_forwarder_pair_records(body, body_start))
    records.sort(key=lambda record: record.source_offset)
    observed_suffixes = [record.suffix for record in records]

    if tuple(observed_suffixes) != EXPECTED_SUFFIXES:
        raise AuditError(
            "paired JNI export inventory changed: expected "
            f"{len(EXPECTED_SUFFIXES)} ordered pairs, found {len(observed_suffixes)}"
        )
    if len(set(observed_suffixes)) != len(observed_suffixes):
        raise AuditError("paired JNI export inventory contains a duplicate suffix")
    literal_records = [record for record in records if not record.generated]
    for record in literal_records:
        sdk_name = SDK_PREFIX + record.suffix
        android_name = ANDROID_PREFIX + record.suffix
        if source.count(sdk_name) != 1 or source.count(android_name) != 1:
            raise AuditError(f"paired JNI export must occur exactly once: {record.suffix}")
        direct_android = f'pub unsafe extern "system" fn {android_name}('
        if direct_android in source:
            raise AuditError(f"Android wrapper escaped the exact pair macro: {android_name}")
    literal_sdk_names = re.findall(
        rf'pub unsafe extern "system" fn ({re.escape(SDK_PREFIX)}[A-Za-z0-9_]+)\(',
        source,
    )
    literal_android_names = re.findall(
        rf'fn ({re.escape(ANDROID_PREFIX)}[A-Za-z0-9_]+)\(\);',
        source,
    )
    if literal_sdk_names != [SDK_PREFIX + record.suffix for record in literal_records]:
        raise AuditError("an SDK JNI wrapper escaped the authenticated pair inventory")
    if literal_android_names != [
        ANDROID_PREFIX + record.suffix for record in literal_records
    ]:
        raise AuditError("an Android JNI wrapper escaped the authenticated pair inventory")

    abi_digest = hashlib.sha256(
        "\0\0".join(sorted(record.abi_record for record in records)).encode()
    ).hexdigest()
    if abi_digest != EXPECTED_ABI_DIGEST:
        raise AuditError(
            "paired JNI signature/body contract changed: "
            f"expected {EXPECTED_ABI_DIGEST}, found {abi_digest}"
        )
    attribute_digest = hashlib.sha256(
        "\0\0".join(sorted(record.attribute_record for record in records)).encode()
    ).hexdigest()
    if attribute_digest != EXPECTED_ATTRIBUTE_DIGEST:
        raise AuditError(
            "paired JNI documentation/attribute contract changed: "
            f"expected {EXPECTED_ATTRIBUTE_DIGEST}, found {attribute_digest}"
        )
    return AuditResult(len(records), abi_digest, attribute_digest)


def main() -> int:
    """Audit the repository JNI source and report the frozen inventory."""

    try:
        if JNI_SOURCE.is_symlink() or not JNI_SOURCE.is_file():
            raise AuditError(f"JNI source is unavailable: {JNI_SOURCE}")
        result = audit_source(JNI_SOURCE.read_text(encoding="utf-8"))
    except (AuditError, OSError, UnicodeError) as error:
        print(f"JNI SDK/Android pair guard failed: {error}", file=sys.stderr)
        return 1
    print(
        "JNI SDK/Android pair guard passed: "
        f"pairs={result.pair_count} abi_sha256={result.abi_digest} "
        f"attributes_sha256={result.attribute_digest}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
