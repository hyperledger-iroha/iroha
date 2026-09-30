#!/usr/bin/env python3
"""Seal the ISO 20022 XML-signature fixture-helper consolidation."""

from __future__ import annotations

import hashlib
import re
import subprocess
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "crates/iroha_torii/src/iso20022_bridge.rs"
PREIMAGE_BLOB = "aff0cbdf71fdb9b300332f394b74fbc781186749"
PREIMAGE_SHA256 = "2bb63a89950628083c29420bdf8abbf4414cd8e698f9da4250d494a6c675abc7"
PREIMAGE_LINES = 21_466
MINIMUM_RUST_LINE_REDUCTION = 300
MAXIMUM_SOURCE_LINES = PREIMAGE_LINES - MINIMUM_RUST_LINE_REDUCTION
MASKED_DONOR_SHA256 = "e01df63f5aa4d67bc9e58230215878b618a566a971d26ddc68067f563b3504d9"

PROTECTED_FUNCTION_SHA256 = {
    "assert_require_verified_signed_payload_accepted":
        "1df595d28f1afaa6390b8afbdc9c729b998104ef27f1cddfd5256ad409a0538c",
    "require_verified_profile_accepts_valid_p256_xmldsig_xades":
        "db82f325221399dc2b8534b2178bc72f417fb0fc19067ea8f1b48259cf575833",
    "require_verified_profile_accepts_xades_signed_properties_reference":
        "606e75175daaf259fa36664053680976f793cf0597a0bd367ef84926e26c5691",
    "require_verified_profile_accepts_prefixed_xades_signed_properties_reference":
        "c87cf5489ff005674d8a5db7934a402812352a8aac8488010bcb6307dd5443ee",
    "require_verified_profile_accepts_comments_in_no_comments_c14n":
        "3c04d74a073b5f2aa5971d7efc044508be962ebb2160f731d9fc02943c879e11",
    "require_verified_profile_accepts_character_references_in_payload_digest":
        "954fd3301bac977725ebc1458adc6be2dc3cac55001761b4972d05f45a0c8964",
    "require_verified_profile_accepts_xml_namespace_attribute_in_payload_digest":
        "5b8f97abebae3c81fce1f97ad10e6290b017f007502d3c9f122615e69f9f96cd",
    "require_verified_profile_accepts_prefixed_attribute_in_payload_digest":
        "3b510437855c2f4a4d2f2a1c1ca635f70fbc7e4a4fb28b4275abbd696841a018",
    "require_verified_profile_accepts_xml_namespace_attribute_in_signed_info":
        "4f969e82c8dbd013574a45ba12e0b05b4de45a0b0e8ef31f62e0ec7cc27ff91a",
    "require_verified_profile_accepts_same_document_id_reference":
        "99327e7c3fc213c40ba6a1ecfd6da556918a10bdf6fc9ae6f08e8260b95774a9",
    "require_verified_profile_accepts_reference_c14n_transform":
        "138930609bb040b251b642fd2e99451383bc9194f6a5de9206ff0e4ff265a5f1",
    "require_verified_profile_accepts_inherited_prefixed_attribute_in_signed_info":
        "7b8bc2ddff7340f465636abc3bdaba1911d0bfc69b4b4c11a51fdff6351eaea0",
    "require_verified_profile_accepts_prefixed_signed_info_with_inherited_namespace":
        "3408c6e1453c261745216f8d281ec69780aceb14f3b5bc79629ba9a19e71fb48",
    "require_verified_profile_accepts_sgntr_wrapped_signature_carrier":
        "b4cb235966a063112c40c15a2c03460306a72bd77a0d5c43611f4f0e82097961",
    "require_verified_profile_accepts_inclusive_c14n_with_unused_inherited_namespace":
        "613f814c305e6abbdd8c01603e20abdb4962846189aceff34a0c0049ce6555d2",
    "require_verified_profile_accepts_fixed_width_ecdsa_signature_value":
        "6b4cc00b8408a222df4ca66789d2fbcd3a0485cacc547066c8cb883cd1d808d4",
    "require_verified_profile_accepts_self_closing_signed_info_methods":
        "bc5e85beaeb24067558cdec5dccf717a48fecb18b12bfb1a44383710654513c1",
    "require_verified_profile_rejects_exclusive_bytes_declared_as_inclusive_c14n":
        "ab0082814bb14a578017a03331edef738bea5e7a7dd77253db77f92b3ae2b101",
    "require_verified_profile_rejects_raw_self_closing_signed_info_signature":
        "e19bc60baa1ace15c6f3d8ce17a30a078fd0dd689eb071f544959129838094d5",
    "require_verified_profile_rejects_public_key_xades_signing_certificate_v2":
        "58a02fa6a0c479ee37f427c88b8c2aa79be11e6c7edbbad5e1053cbf6538571b",
    "require_verified_profile_rejects_unsupported_signature_method":
        "7288af2278a42339b216c38a65647d186ba244514550ac447cfbdc550c1c0056",
    "require_verified_profile_rejects_extra_reference_transform":
        "739c3bcb1959809d3537d120cc088ee70b8b39f22bfd9b91126a612da8a90a8e",
    "require_verified_profile_rejects_payload_digest_tampering":
        "625dd4ce2d437c0f554e9fe7fdcb47a8671d262016e56268db1ab067bff11502",
    "require_verified_profile_rejects_signed_properties_digest_tampering":
        "3cf02a83dd5dd8010a079dcfce2eba5ad9c4eb158962bdb14f59bb1273d93b83",
    "require_verified_profile_rejects_signed_properties_target_drift":
        "20f7627acef840d4dbc7372681449bd9a63fbf95ae914328aea266afbdf0e737",
    "require_verified_profile_rejects_signature_value_tampering":
        "ce087c8b1ae927140d7b0bc20ee79b2a6b5e419d5fa44535053106b3d8774bf7",
    "require_verified_profile_rejects_duplicate_signature_value":
        "8effbed19667ffb0825e96d89360caf5fdedf6a973e6be701110b5a24d86fe4a",
    "require_verified_profile_rejects_duplicate_digest_value":
        "82c022a10a678ac2b31c0c73c3d8282f9d838cde738fae02553284d92a17cdfd",
    "require_verified_profile_rejects_duplicate_public_key":
        "225a4b7cda9ba83aa3b1657250aab463a0c546f85950ef29c3d8d45f8fdb575e",
}

FORBIDDEN_PROTECTED_TOKENS = (
    "Box<dyn Fn",
    "dyn Fn",
    "impl Fn",
    "FnMut",
    "FnOnce",
    "macro_rules!",
    "$body",
    "$setup",
    "enum Action",
    "enum Step",
    "enum Scenario",
)


class GuardError(AssertionError):
    """Raised when the protected source contract drifts."""


def _sha256(data: bytes | str) -> str:
    if isinstance(data, str):
        data = data.encode()
    return hashlib.sha256(data).hexdigest()


def _normalized_hash(source: str) -> str:
    return _sha256(re.sub(r"\s+", " ", source).strip())


def _preimage() -> str:
    result = subprocess.run(
        ["git", "cat-file", "blob", PREIMAGE_BLOB],
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
    )
    if _sha256(result.stdout) != PREIMAGE_SHA256:
        raise GuardError("ISO bridge donor blob digest changed")
    if len(result.stdout.splitlines()) != PREIMAGE_LINES:
        raise GuardError("ISO bridge donor line count changed")
    return result.stdout.decode()


def _skip_rust_non_code(source: str, index: int) -> int | None:
    if source.startswith("//", index):
        end = source.find("\n", index)
        return len(source) if end < 0 else end
    if source.startswith("/*", index):
        depth = 1
        cursor = index + 2
        while cursor < len(source):
            if source.startswith("/*", cursor):
                depth += 1
                cursor += 2
            elif source.startswith("*/", cursor):
                depth -= 1
                cursor += 2
                if depth == 0:
                    return cursor
            else:
                cursor += 1
        return len(source)
    raw = re.match(r'(?:br|r)(?P<hashes>#{0,255})"', source[index:])
    if raw:
        terminator = '"' + raw.group("hashes")
        end = source.find(terminator, index + raw.end())
        return len(source) if end < 0 else end + len(terminator)
    if source[index : index + 1] not in {'"', "'"}:
        return None
    quote = source[index]
    cursor = index + 1
    while cursor < len(source):
        if source[cursor] == "\\":
            cursor += 2
        elif source[cursor] == quote:
            return cursor + 1
        else:
            cursor += 1
    return len(source)


def _matching_brace(source: str, opening: int) -> int:
    depth = 1
    cursor = opening + 1
    while cursor < len(source):
        skipped = _skip_rust_non_code(source, cursor)
        if skipped is not None:
            cursor = skipped
            continue
        if source[cursor] == "{":
            depth += 1
        elif source[cursor] == "}":
            depth -= 1
            if depth == 0:
                return cursor
        cursor += 1
    raise GuardError("unterminated protected Rust function")


def _function_span(source: str, name: str) -> tuple[int, int]:
    matches = list(re.finditer(rf"(?m)^\s*fn\s+{re.escape(name)}\b", source))
    if len(matches) != 1:
        raise GuardError(f"{name}: expected exactly one function")
    opening = source.find("{", matches[0].end())
    if opening < 0:
        raise GuardError(f"{name}: missing function body")
    return matches[0].start(), _matching_brace(source, opening) + 1


def _function(source: str, name: str) -> str:
    start, end = _function_span(source, name)
    return source[start:end]


def _masked_source(source: str) -> str:
    spans = [
        (*_function_span(source, name), name)
        for name in PROTECTED_FUNCTION_SHA256
    ]
    for start, end, name in sorted(spans, reverse=True):
        source = source[:start] + f"\n<FUNCTION {name}>\n" + source[end:]
    return source


def _test_inventory(source: str) -> tuple[tuple[tuple[str, ...], str], ...]:
    inventory: list[tuple[tuple[str, ...], str]] = []
    attributes: list[str] = []
    for line in source.splitlines():
        stripped = line.strip()
        if stripped.startswith("#["):
            attributes.append(stripped)
            continue
        match = re.match(r"(?:async\s+)?fn\s+([A-Za-z0-9_]+)\b", stripped)
        if match and any(
            attribute == "#[test]" or attribute.startswith("#[tokio::test")
            for attribute in attributes
        ):
            inventory.append((tuple(attributes), match.group(1)))
        if stripped:
            attributes = []
    return tuple(inventory)


def validate_source(source: str, donor: str) -> None:
    if len(source.splitlines()) > MAXIMUM_SOURCE_LINES:
        raise GuardError("ISO signature fixture consolidation lost its 300-line ratchet")
    if _test_inventory(source) != _test_inventory(donor):
        raise GuardError("ISO bridge ordered test names or attributes changed")
    if _sha256(_masked_source(donor)) != MASKED_DONOR_SHA256:
        raise GuardError("ISO bridge masked donor projection changed")
    if _sha256(_masked_source(source)) != MASKED_DONOR_SHA256:
        raise GuardError("ISO bridge source outside the protected fixture functions changed")
    protected = "\n".join(
        _function(source, name) for name in PROTECTED_FUNCTION_SHA256
    )
    for token in FORBIDDEN_PROTECTED_TOKENS:
        if token in protected:
            raise GuardError(f"ISO bridge helper gained escape hatch {token!r}")
    for name, expected in PROTECTED_FUNCTION_SHA256.items():
        if _normalized_hash(_function(source, name)) != expected:
            raise GuardError(f"{name}: protected fixture/assertion sequence changed")


class Iso20022BridgeSignatureFixtureSourceTest(unittest.TestCase):
    def test_source_contract(self) -> None:
        validate_source(SOURCE.read_text(), _preimage())

    def test_mutations_fail_closed(self) -> None:
        source = SOURCE.read_text()
        donor = _preimage()
        mutations = (
            source.replace(
                "assert!(metadata.embedded_signature_detected());",
                "assert!(!metadata.embedded_signature_detected());",
                1,
            ),
            source.replace('Some("sig&001")', 'Some("sig?001")', 1),
            source.replace(
                "signed_pacs008_xml_with_comments()",
                "signed_pacs008_xml()",
                1,
            ),
            source.replace(
                "reference digest mismatch must fail closed",
                "reference digest mismatch was ignored",
                1,
            ),
            source.replace(
                "fn require_verified_profile_rejects_duplicate_public_key()",
                "fn require_verified_profile_rejects_duplicate_public_keys()",
                1,
            ),
            source.replace(
                "    #[test]\n    fn require_verified_profile_accepts_valid_p256_xmldsig_xades",
                "    fn require_verified_profile_accepts_valid_p256_xmldsig_xades",
                1,
            ),
            source.replace(
                "expected_business_message_id: Option<&str>,",
                "expected_business_message_id: Option<&str>,\n        _callback: impl FnOnce(),",
                1,
            ),
            source + "\n// unauthorized ISO bridge source drift\n",
            source + "\n" * (MINIMUM_RUST_LINE_REDUCTION + 1),
        )
        for mutated in mutations:
            with self.assertRaises(GuardError):
                validate_source(mutated, donor)


if __name__ == "__main__":
    unittest.main()
