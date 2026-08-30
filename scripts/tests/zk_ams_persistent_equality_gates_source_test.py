#!/usr/bin/env python3
"""Seal the remaining ZK-AMS persistent/ciphertext equality gates.

The guard records two intentionally different outcomes:

* audit bit 7 remains irreducible under the frozen direct-language proof and
  work caps; no proof, verifier, receipt, or availability claim may appear;
* bit 8 gains only a real typed input handoff from the exact 43 source-record
  receipts to one complete RNS-native public transcript.  The input is
  move-only and non-authorizing; ciphertext equations, persistent equality,
  and release all remain false.

Mutation tests operate only on in-memory strings.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
VEGA = ROOT / "crates/iroha_zkp_halo2/src/vega"
MKHE = VEGA / "zk_ams/mkhe"
PATHS = {
    "direct": MKHE / "persistent_decryption_direct_equality_v1.rs",
    "source": MKHE / "rns_native_split_decryption_source_v2.rs",
    "audit": MKHE / "receipt_capability_audit.rs",
    "mkhe": VEGA / "zk_ams/mkhe.rs",
    "zk_ams": VEGA / "zk_ams.rs",
    "vega": VEGA / "mod.rs",
}
# This checkout stores the facade in `vega.rs`, not `vega/mod.rs`.
PATHS["vega"] = VEGA.parent / "vega.rs"

INPUT = "ZkAmsMkheRnsNativeCiphertextEqualityInputV2"
BINDER = "bind_zk_ams_mkhe_rns_native_ciphertext_equality_input_v2"


def load_sources() -> dict[str, str]:
    return {name: path.read_text(encoding="utf-8") for name, path in PATHS.items()}


def require(source: str, marker: str, message: str) -> None:
    if marker not in source:
        raise AssertionError(message)


def region(source: str, start: str, end: str) -> str:
    if start not in source or end not in source.split(start, 1)[1]:
        raise AssertionError(f"missing source boundary {start!r} .. {end!r}")
    return source.split(start, 1)[1].split(end, 1)[0]


def validate_contract(sources: dict[str, str]) -> None:
    direct = sources["direct"]
    source = sources["source"]
    audit = sources["audit"]

    # The only currently specified direct proof that reaches the 128-bit
    # target needs nine rounds and exceeds both independent release caps.
    direct_markers = (
        "const DIRECT_PROOF_CAP_BYTES_V1: u64 = 33_554_432;",
        "const RELEASE_WORK_CAP_V1: u64 = 100_000_000_000;",
        "const FALLBACK_NINE_ROUND_BYTES_V1: u64 = 297_296_163;",
        "const FALLBACK_NINE_ROUND_WORK_V1: u64 = 625_432_370_841;",
        "const FALLBACK_NINE_ROUND_BITS_HUNDREDTHS_V1: u32 = 13_284;",
        "const FALLBACK_TARGET_BITS_HUNDREDTHS_V1: u32 = 12_800;",
        "const THEOREM_PINNED_V1: bool = false;",
        "const CIRCUIT_PINNED_V1: bool = false;",
        "const BACKEND_IMPLEMENTED_V1: bool = false;",
        "const DIRECT_EQUALITY_VERIFIED_V1: bool = false;",
        "const PERSISTENT_DECRYPTION_AUDIT_BIT_7_CLOSED_V1: bool = false;",
        "const RELEASE_READY_V1: bool = false;",
        "assert!(FALLBACK_NINE_ROUND_BITS_HUNDREDTHS_V1 >= FALLBACK_TARGET_BITS_HUNDREDTHS_V1);",
        "assert!(!ZERO_KNOWLEDGE_ACCEPTED_V1 && !PERSISTENT_DECRYPTION_AUDIT_BIT_7_CLOSED_V1);",
    )
    for marker in direct_markers:
        require(direct, marker, f"bit-7 cap/gate marker changed: {marker}")
    for forbidden in (
        "PersistentDecryptionDirectEqualityProofV1",
        "VerifiedPersistentDecryptionDirectEqualityReceiptV1",
        "fn verify_direct_equality",
    ):
        if forbidden in direct:
            raise AssertionError(f"bit 7 acquired an uncertified production surface: {forbidden}")

    require(source, "struct CiphertextEqualityInputSealV2;", "bit-8 input seal must be private")
    require(source, f"pub struct {INPUT} {{", "typed equality input disappeared")
    if "pub struct CiphertextEqualityInputSealV2" in source:
        raise AssertionError("the equality-input seal became externally constructible")
    reusable_derive = re.compile(
        rf"#\[derive\([^\]]*(?:Clone|Copy|Default|Encode|Decode|Serialize|Deserialize)"
        rf"[^\]]*\)\]\s*(?:#\[[^\]]+\]\s*)*pub struct {INPUT}"
    )
    reusable_impl = re.compile(
        rf"impl\s+(?:[A-Za-z0-9_:]+::)*(?:Clone|Copy|Default|Encode|Decode|Serialize|Deserialize)"
        rf"\s+for\s+{INPUT}\b"
    )
    if reusable_derive.search(source) or reusable_impl.search(source):
        raise AssertionError("the equality input became reusable or serializable")
    if source.count("_seal: CiphertextEqualityInputSealV2") != 2:
        raise AssertionError("the equality input has a missing or duplicate minting path")

    binder = region(source, f"pub fn {BINDER}(", "fn validate_ciphertext_equality_input_context_v2")
    binder_markers = (
        "source_seal_receipt: ZkAmsMkheRnsNativeSplitDecryptionSourceSealReceiptV2",
        "ZK_AMS_MKHE_RNS_NATIVE_SPLIT_DECRYPTION_RECORD_COUNT_V2",
        "transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1",
        "validate_ciphertext_equality_input_context_v2(layout, authority, source_receipt, transcript)?;",
        "source_seal_receipt.validate(layout, authority, source_receipt)?;",
        "validate_ciphertext_equality_record_receipts_v2(record_receipts.as_ref(), layout, authority)?;",
        "ordered_record_root != source_seal_receipt.ordered_record_root",
        "ordered_ciphertext_identity_root_v2(",
        "ciphertext_equations_verified: false,\n        persistent_equality_verified: false,\n        release_available: false,",
        "input.validate(layout, authority, source_receipt, transcript)?;",
    )
    for marker in binder_markers:
        require(binder, marker, f"bit-8 binder lost marker: {marker}")
    if "source_seal_receipt: &" in binder or "record_receipts: &" in binder:
        raise AssertionError("the bit-8 binder borrowed replayable input containers")

    order = [
        binder.find("validate_ciphertext_equality_input_context_v2"),
        binder.find("source_seal_receipt.validate"),
        binder.find("validate_ciphertext_equality_record_receipts_v2"),
        binder.find("ordered_record_root != source_seal_receipt.ordered_record_root"),
        binder.find(f"let mut input = {INPUT}"),
        binder.find("input.validate"),
    ]
    if min(order) < 0 or order != sorted(order):
        raise AssertionError("the bit-8 input can be minted before all exact-set checks")

    validator = region(source, f"impl {INPUT} {{", f"pub fn {BINDER}(")
    for marker in (
        "validate_ciphertext_equality_record_receipts_v2(self.records.as_ref(), layout, authority)?;",
        "|| self.ciphertext_equations_verified",
        "|| self.persistent_equality_verified",
        "|| self.release_available",
        "self.input_digest != ciphertext_equality_input_digest_v2(self)",
    ):
        require(validator, marker, f"bit-8 input validation lost marker: {marker}")
    receipt_validator = region(
        source,
        "fn validate_ciphertext_equality_record_receipts_v2(",
        "fn ordered_record_root_from_receipts_v2",
    )
    for marker in (
        "record.validate(layout, authority)?;",
        "usize::from(record.record_index) != record_index",
        "prior.receipt_digest == record.receipt_digest",
        "prior.ciphertext_digest == record.ciphertext_digest",
    ):
        require(receipt_validator, marker, f"exact record-set validation lost marker: {marker}")
    digest = region(source, "fn ciphertext_equality_input_digest_v2(", "/// Move-only sealed snapshot")
    for marker in (
        "input.ciphertext_equations_verified.into()",
        "input.persistent_equality_verified.into()",
        "input.release_available.into()",
    ):
        require(digest, marker, f"bit-8 false gate is not digest-bound: {marker}")

    # The canonical audit keeps both equality blockers and aggregate release
    # false. The new primitive is explicitly only an input handoff.
    production_audit = region(
        audit,
        "pub(super) fn zk_ams_mkhe_receipt_capability_audit_v1",
        "pub(super) fn require_zk_ams_mkhe_receipt_capability_v1",
    )
    for marker in (
        "persistent_decryption_equality_complete: false",
        "split_decryption_source_ciphertext_equality_complete: false",
        "release_available: false",
        "closes only\n        // the verifier-input handoff, not the bit-8 equality requirement",
    ):
        require(production_audit, marker, f"equality audit was weakened: {marker}")
    require(
        audit,
        "const _: () = assert!(CURRENT_OPEN_RECEIPT_CAPABILITY_BLOCKERS_V1 == 0x1f0);",
        "bits 4-8 must remain open",
    )

    for facade in ("mkhe", "zk_ams", "vega"):
        require(
            sources[facade],
            "ZK_AMS_MKHE_RNS_NATIVE_SPLIT_DECRYPTION_RECORD_COUNT_V2",
            f"{facade} does not expose the exact-43 public count",
        )
        require(sources[facade], INPUT, f"{facade} does not expose the typed equality input")
        require(sources[facade], BINDER, f"{facade} does not expose the typed equality binder")


class PersistentEqualityGateSourceTests(unittest.TestCase):
    def setUp(self) -> None:
        self.sources = load_sources()

    def test_live_source_contract(self) -> None:
        validate_contract(self.sources)

    def reject(self, name: str, old: str, new: str) -> None:
        mutated = dict(self.sources)
        if old not in mutated[name]:
            self.fail(f"mutation preimage missing in {name}: {old!r}")
        mutated[name] = mutated[name].replace(old, new, 1)
        with self.assertRaises(AssertionError):
            validate_contract(mutated)

    def test_mutations_fail_closed(self) -> None:
        mutations = (
            ("direct", "const BACKEND_IMPLEMENTED_V1: bool = false;", "const BACKEND_IMPLEMENTED_V1: bool = true;"),
            ("direct", "const FALLBACK_NINE_ROUND_BYTES_V1: u64 = 297_296_163;", "const FALLBACK_NINE_ROUND_BYTES_V1: u64 = 33_000_000;"),
            ("source", f"pub struct {INPUT} {{", f"#[derive(Clone)]\npub struct {INPUT} {{"),
            (
                "source",
                "source_seal_receipt: ZkAmsMkheRnsNativeSplitDecryptionSourceSealReceiptV2",
                "source_seal_receipt: &ZkAmsMkheRnsNativeSplitDecryptionSourceSealReceiptV2",
            ),
            (
                "source",
                "source_seal_receipt.validate(layout, authority, source_receipt)?;",
                "// source seal validation removed",
            ),
            ("source", "record.validate(layout, authority)?;", "// record validation removed"),
            (
                "source",
                "prior.ciphertext_digest == record.ciphertext_digest",
                "false",
            ),
            (
                "source",
                "ciphertext_equations_verified: false,\n        persistent_equality_verified: false,\n        release_available: false,",
                "ciphertext_equations_verified: true,\n        persistent_equality_verified: true,\n        release_available: true,",
            ),
            (
                "source",
                "|| self.ciphertext_equations_verified",
                "|| false /* equality flag ignored */",
            ),
            (
                "audit",
                "persistent_decryption_equality_complete: false",
                "persistent_decryption_equality_complete: true",
            ),
            (
                "audit",
                "split_decryption_source_ciphertext_equality_complete: false",
                "split_decryption_source_ciphertext_equality_complete: true",
            ),
            ("audit", "release_available: false", "release_available: true"),
        )
        for name, old, new in mutations:
            with self.subTest(name=name, old=old):
                self.reject(name, old, new)


if __name__ == "__main__":
    unittest.main()
