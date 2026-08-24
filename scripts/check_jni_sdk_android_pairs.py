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
MACRO_NAME = "jni_sdk_android_pairs"
EXPECTED_MACRO_DIGEST = "11de37a9ba22a1c17f322b2803478a5670ecc17166b23526f080c54fbb8a84bb"
EXPECTED_ABI_DIGEST = "fa65747adf061879da05b24aeea39d04f3476db289ac590171225ba15a9b491a"
EXPECTED_ATTRIBUTE_DIGEST = (
    "aec610f58c5bc0aaa9d05c7ae73730bc6ae2535c72a2e1b5643a0f446c707eaf"
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
        "nativeBuildTopUpProvenanceV4",
        "nativeValidateTopUpProvenanceV4",
        "nativeBuildAppendRequestV4",
        "nativeBuildVerifyRequestV4",
        "nativeBuildRedeemRequestV4",
        "nativeProjectPeerPaymentV4",
        "nativeProjectInitResultV4",
        "nativeProjectSplitResultV4",
        "nativeProjectVerifyResultV4",
        "nativeProjectRedeemBuildResultV4",
        "nativePrepareAcknowledgementV2",
        "nativeCreateAcknowledgementV2",
        "nativeVerifyAcknowledgementV2",
        "nativePrepareAuthorizationV2",
        "nativeFinalizeHardwareAuthorizationV2",
        "nativeFinalizeIosAppAttestAuthorizationV2",
        "nativeFinalizeTopUpV4",
        "nativeFinalizeRedeemV4",
        "nativePrepareTopUpV4",
        "nativeProjectOperationStatusV4",
        "nativeBranchClaimsConflictV2",
        "nativePrepareRedemptionChangeV4",
        "nativePreparePeerSplitChangeV4",
        "nativePrepareNoteOpeningV2",
        "nativeProjectRecipientRequestV2",
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


def _macro_invocation(source: str) -> tuple[str, int, int]:
    """Return the paired macro body and its source boundaries."""

    definition_start = source.find(f"macro_rules! {MACRO_NAME}")
    invocation_start = source.find(f"{MACRO_NAME}! {{")
    if definition_start != 0 or invocation_start < 0:
        raise AuditError("paired JNI macro definition and invocation must lead part_3.rs")
    macro_definition = source[definition_start:invocation_start]
    macro_digest = hashlib.sha256(" ".join(macro_definition.split()).encode()).hexdigest()
    if macro_digest != EXPECTED_MACRO_DIGEST:
        raise AuditError(
            "paired JNI macro expansion contract changed: "
            f"expected {EXPECTED_MACRO_DIGEST}, found {macro_digest}"
        )
    opening = source.index("{", invocation_start)
    closing = _matching_brace(source, opening)
    return source[opening + 1 : closing], opening + 1, closing


def audit_source(source: str) -> AuditResult:
    """Validate the exact paired export, signature, body, and attribute inventory."""

    body, _body_start, _body_end = _macro_invocation(source)
    cursor = 0
    observed_suffixes = []
    abi_records = []
    attribute_records = []
    while True:
        cursor += len(body[cursor:]) - len(body[cursor:].lstrip())
        if cursor == len(body):
            break
        if not body.startswith("android:", cursor):
            raise AuditError(f"unexpected token before paired wrapper {len(observed_suffixes) + 1}")
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
        observed_suffixes.append(sdk_suffix)
        abi_records.append(
            sdk_suffix + "\0" + function_item.replace(sdk_name, "__JNI_EXPORT__", 1)
        )
        attribute_records.append(
            sdk_suffix
            + "\0"
            + sdk_attributes
            + "\0"
            + android_attributes
            + "#[unsafe(no_mangle)]\n"
        )
        cursor = body_close + 1

    if tuple(observed_suffixes) != EXPECTED_SUFFIXES:
        raise AuditError(
            "paired JNI export inventory changed: expected "
            f"{len(EXPECTED_SUFFIXES)} ordered pairs, found {len(observed_suffixes)}"
        )
    for suffix in EXPECTED_SUFFIXES:
        sdk_name = SDK_PREFIX + suffix
        android_name = ANDROID_PREFIX + suffix
        if source.count(sdk_name) != 1 or source.count(android_name) != 1:
            raise AuditError(f"paired JNI export must occur exactly once: {suffix}")
        direct_android = f'pub unsafe extern "system" fn {android_name}('
        if direct_android in source:
            raise AuditError(f"Android wrapper escaped the exact pair macro: {android_name}")

    abi_digest = hashlib.sha256("\0\0".join(sorted(abi_records)).encode()).hexdigest()
    if abi_digest != EXPECTED_ABI_DIGEST:
        raise AuditError(
            "paired JNI signature/body contract changed: "
            f"expected {EXPECTED_ABI_DIGEST}, found {abi_digest}"
        )
    attribute_digest = hashlib.sha256(
        "\0\0".join(sorted(attribute_records)).encode()
    ).hexdigest()
    if attribute_digest != EXPECTED_ATTRIBUTE_DIGEST:
        raise AuditError(
            "paired JNI documentation/attribute contract changed: "
            f"expected {EXPECTED_ATTRIBUTE_DIGEST}, found {attribute_digest}"
        )
    return AuditResult(len(observed_suffixes), abi_digest, attribute_digest)


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
