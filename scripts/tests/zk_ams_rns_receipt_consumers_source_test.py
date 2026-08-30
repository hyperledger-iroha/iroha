#!/usr/bin/env python3
"""Fail closed on the ZK-AMS RNS receipt-to-consumer capability graph.

This stdlib-only guard deliberately does not execute a proof kernel.  It seals
the source invariants that must hold while that kernel remains unavailable:
only a consumed opaque algebraic receipt can mint the move-only terminal or
party-indexed uses, every live prover burns its use before a side effect, all
four replacement-profile producers still fail with ``StageUnavailable``, and
the transitive Exact12 availability bits remain open.

Mutation cases operate on in-memory source copies and never modify the tree.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MKHE = ROOT / "crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe"

PATHS = {
    "module": MKHE.parent / "mkhe.rs",
    "consumer": MKHE / "rns_native_receipt_consumers.rs",
    "composite": MKHE / "rns_native_composite_verifier.rs",
    "decryption": MKHE / "decryption_streaming.rs",
    "terminal": MKHE / "terminal.rs",
    "audit": MKHE / "receipt_capability_audit.rs",
    "zk_ams_facade": MKHE.parent.parent / "zk_ams.rs",
    "vega_facade": MKHE.parents[2] / "vega.rs",
}

SPLIT = "ZkAmsMkheRnsNativeSplitDecryptionUseV1"
TERMINAL = "ZkAmsMkheRnsNativeTerminalMaterializationUseV1"
RECEIPT = "ZkAmsMkheRnsNativeAlgebraicReceiptV1"
TRANSPORT = "ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1"


def load_sources() -> dict[str, str]:
    return {name: path.read_text(encoding="utf-8") for name, path in PATHS.items()}


def require(source: str, marker: str, message: str) -> None:
    if marker not in source:
        raise AssertionError(message)


def function_region(source: str, start: str, end: str) -> str:
    try:
        tail = source.split(start, 1)[1]
        return tail.split(end, 1)[0]
    except (IndexError, ValueError) as error:
        raise AssertionError(f"missing function boundary: {start!r} .. {end!r}") from error


def assert_move_only(source: str, type_name: str) -> None:
    require(source, f"pub struct {type_name} {{", f"{type_name} must remain public and sealed")
    forbidden_derive = re.compile(
        rf"#\[derive\([^\]]*(?:Clone|Copy|Default|Encode|Decode|Serialize|Deserialize)"
        rf"[^\]]*\)\]\s*(?:#\[[^\]]+\]\s*)*pub struct {re.escape(type_name)}"
    )
    if forbidden_derive.search(source):
        raise AssertionError(f"{type_name} acquired a reusable or serializable derive")
    forbidden_impl = re.compile(
        rf"impl\s+(?:[A-Za-z0-9_:]+::)*(?:Clone|Copy|Default|Encode|Decode|Serialize|Deserialize)"
        rf"\s+for\s+{re.escape(type_name)}\b"
    )
    if forbidden_impl.search(source):
        raise AssertionError(f"{type_name} acquired a reusable or serializable trait")


def validate_contract(sources: dict[str, str]) -> None:
    module = sources["module"]
    consumer = sources["consumer"]
    composite = sources["composite"]
    decryption = sources["decryption"]
    terminal = sources["terminal"]
    audit = sources["audit"]

    require(
        module,
        '#[path = "mkhe/rns_native_receipt_consumers.rs"]\nmod rns_native_receipt_consumers;',
        "the checked receipt-consumer module must be wired",
    )
    require(consumer, "struct SplitDecryptionUseSealV1;", "split use seal must stay private")
    require(
        consumer,
        "struct TerminalMaterializationUseSealV1;",
        "terminal use seal must stay private",
    )
    if "pub struct SplitDecryptionUseSealV1" in consumer or "pub struct TerminalMaterializationUseSealV1" in consumer:
        raise AssertionError("a receipt-use seal became externally constructible")
    assert_move_only(consumer, SPLIT)
    assert_move_only(consumer, TERMINAL)

    # Each seal occurs once as a private field type and once at the sole
    # receipt-consuming construction site. Extra occurrences are new minting
    # paths and require explicit review of this guard.
    if consumer.count("_seal: SplitDecryptionUseSealV1") != 2:
        raise AssertionError("split-decryption use has a missing or duplicate minting path")
    if consumer.count("_seal: TerminalMaterializationUseSealV1") != 2:
        raise AssertionError("terminal use has a missing or duplicate minting path")

    split_binder = function_region(
        consumer,
        "pub fn bind_zk_ams_mkhe_rns_native_split_decryption_uses_v1(",
        "fn split_decryption_use_digest_v1",
    )
    require(split_binder, f"receipt: {RECEIPT}", "split binder must consume the opaque receipt")
    if f"receipt: &{RECEIPT}" in split_binder:
        raise AssertionError("split binder borrowed a reusable receipt")
    require(
        split_binder,
        f"[{SPLIT}; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1]",
        "split binder must mint the complete fixed party set atomically",
    )
    require(
        split_binder,
        "receipt\n        .validate_v1()",
        "split binder must revalidate its receipt",
    )
    require(
        split_binder,
        "core::array::from_fn(|party_index|",
        "split binder must bind every governed party index",
    )

    terminal_binder = function_region(
        consumer,
        "pub fn bind_zk_ams_mkhe_rns_native_terminal_materialization_use_v1(",
        "fn terminal_materialization_use_digest_v1",
    )
    require(
        terminal_binder,
        f"receipt: {RECEIPT}",
        "terminal binder must consume the opaque receipt",
    )
    if f"receipt: &{RECEIPT}" in terminal_binder:
        raise AssertionError("terminal binder borrowed a reusable receipt")
    require(
        terminal_binder,
        "receipt\n        .validate_v1()",
        "terminal binder must revalidate its receipt",
    )
    require(
        terminal_binder,
        "zk_ams_mkhe_rns_native_terminal_materialization_binding_v1(context, materialized)?",
        "terminal use must bind the exact materialization and replay context",
    )

    staged = function_region(
        decryption,
        "pub fn prove_zk_ams_mkhe_decryption_share_staged_v1",
        "/// Zero-copy canonical view",
    )
    require(staged, f"rns_link_use: {SPLIT}", "staged decryption must own one RNS use")
    split_consume = staged.find("rns_link_use.consume_for_split_decryption_v1")
    persistent_consume = staged.find("statement.consume_party_use_v1")
    first_random = staged.find("let mut bounded_random")
    first_publish = staged.find("publish_staged_share_polynomial_v1")
    if min(split_consume, persistent_consume, first_random, first_publish) < 0:
        raise AssertionError("staged decryption consumption/side-effect markers are incomplete")
    if not split_consume < persistent_consume < first_random < first_publish:
        raise AssertionError("staged decryption does not burn capabilities before RNG/CAS work")

    public_terminal = function_region(
        terminal,
        "pub fn prove_zk_ams_phase3_terminal_v1",
        "pub fn verify_zk_ams_phase3_terminal_v1",
    )
    require(
        public_terminal,
        f"rns_link_use: {TERMINAL}",
        "terminal prover must own one RNS materialization use",
    )
    readiness = public_terminal.find("super::require_release_ready_v1()?")
    terminal_consume = public_terminal.find("rns_link_use.consume_for_terminal_materialization_v1")
    proof = public_terminal.find("prove_terminal_inner(")
    if min(readiness, terminal_consume, proof) < 0 or not readiness < terminal_consume < proof:
        raise AssertionError("terminal prover can bypass readiness or receipt consumption")

    # Receipt construction remains sealed behind the consumed composite
    # candidate, and all four incomplete producers stay explicitly unavailable.
    require(
        composite,
        f"pub struct {RECEIPT} {{",
        "the opaque algebraic receipt definition disappeared",
    )
    assert_move_only(composite, RECEIPT)
    require(
        composite,
        f"pub struct {TRANSPORT} {{",
        "the verifier-authenticated transport definition disappeared",
    )
    assert_move_only(composite, TRANSPORT)
    require(
        composite,
        "pub fn authenticate_canonical_exact_v1(",
        "the bounded canonical transport authenticator disappeared",
    )
    for verifier in (
        "pub fn verify_zk_ams_mkhe_rns_native_composite_v1(",
        "pub fn verify_zk_ams_mkhe_rns_native_algebraic_v1(",
    ):
        signature = function_region(composite, verifier, ") -> Result")
        require(
            signature,
            f"transport: {TRANSPORT}",
            "a production verifier stopped requiring the sealed transport",
        )
        for raw_input in (
            "ZkAmsMkheRnsNativeProofEnvelopeV1",
            "ZkAmsMkheRnsNativeSourceLayoutV1",
            "ZkAmsMkheRnsNativeSourceReceiptV1",
            "ZkAmsMkheRnsNativeChallengeSeedsV1",
        ):
            if raw_input in signature:
                raise AssertionError("a production verifier reacquired caller-owned context")
    for binding in (
        "verifier_context_digest: receipt.verifier_context_digest()",
        "opening_commitment_root: receipt.opening_commitment_root()",
        "verifier_transport_digest: receipt.verifier_transport_digest()",
    ):
        require(
            consumer,
            binding,
            "a production receipt consumer lost its authenticated transport binding",
        )
    require(
        composite,
        "fn from_verified_composite_v1(\n        composite: ZkAmsMkheRnsNativeCompositeCandidateReceiptV1,",
        "receipt minting must consume the verified composite candidate",
    )
    unavailable = (
        "TerminalHyraxBpBridge",
        "RnsRelationQpcs",
        "CrossFieldGlobalLookup",
        "ZeroPadding",
    )
    for stage in unavailable:
        require(
            composite,
            "ZkAmsMkheRnsNativeCompositeVerificationErrorV1::StageUnavailable(\n"
            f"            ZkAmsMkheRnsNativeVerificationStageV1::{stage},",
            f"{stage} producer must remain StageUnavailable",
        )
    if composite.count("ZkAmsMkheRnsNativeCompositeVerificationErrorV1::StageUnavailable(\n") != 4:
        raise AssertionError("the production composite verifier changed its unavailable-stage set")

    # The local handoff plumbing is implemented, while every transitive proof
    # blocker and the aggregate release decision remain fail closed.
    require(
        audit,
        "const _: () = assert!(CURRENT_OPEN_RECEIPT_CAPABILITY_BLOCKERS_V1 == 0x1f0);",
        "the five transitive receipt blockers must remain open",
    )
    for marker in (
        "const _: () = assert!(ALL_IMPLEMENTATION_PREREQUISITES_V1 == 0x7f);",
        "const _: () = assert!(CURRENT_MISSING_IMPLEMENTATION_PREREQUISITES_V1 == 0x7c);",
        "& !(PREREQUISITE_VERIFIER_DERIVED_RNS_TRANSPORT_V1",
        "| PREREQUISITE_PRODUCTION_NATIVE_BGV_OPENING_RECEIPT_V1)",
        "self.validate_logical_graph_v1()?;\n        self.validate_current_implementation_v1()",
        "fn implementation_prerequisite_blocker_mask_v1(",
        "PREREQUISITE_VERIFIER_DERIVED_RNS_TRANSPORT_V1",
        "PREREQUISITE_PRODUCTION_NATIVE_BGV_OPENING_RECEIPT_V1",
        "PREREQUISITE_CORRECTED_RNS_GEOMETRY_END_TO_END_V1",
        "PREREQUISITE_RNS_CARRY_QUOTIENT_SOURCE_RELATION_V1",
        "PREREQUISITE_HYRAX_SOURCE_MATERIALIZATION_EQUALITY_V1",
        "PREREQUISITE_PERSISTENT_DECRYPTION_SIS_CERTIFICATE_V1",
        "PREREQUISITE_SPLIT_SOURCE_CIPHERTEXT_EQUATIONS_V1",
    ):
        require(
            audit,
            marker,
            f"the exact production-prerequisite inventory lost {marker!r}",
        )
    audit_production = function_region(
        audit,
        "pub(super) fn zk_ams_mkhe_receipt_capability_audit_v1",
        "pub(super) fn require_zk_ams_mkhe_receipt_capability_v1",
    )
    required_audit_markers = (
        "rns_link_transport_bound: true",
        "rns_link_family_geometry_matches_native: false",
        "rns_link_carry_quotient_responses_verifiable: false",
        "hyrax_bgv_equality_responses_verifiable: false",
        "terminal_materialization_receipt_enforced: true",
        "split_decryption_receipts_enforced: true",
        "persistent_decryption_equality_complete: false",
        "split_decryption_source_ciphertext_equality_complete: false",
        "release_available: false",
    )
    for marker in required_audit_markers:
        require(
            audit_production,
            marker,
            f"receipt-capability audit lost fail-closed marker {marker!r}",
        )

    # Both public facades must propagate every use and binder. Without this
    # reachability the private mkhe reexport regresses into strict unused/dead
    # code warnings and callers cannot supply the required public parameters.
    public_surface = (
        TRANSPORT,
        "ZK_AMS_MKHE_RNS_NATIVE_VERIFIER_TRANSPORT_VERSION_V1",
        SPLIT,
        TERMINAL,
        "bind_zk_ams_mkhe_rns_native_split_decryption_uses_v1",
        "bind_zk_ams_mkhe_rns_native_terminal_materialization_use_v1",
        "zk_ams_mkhe_rns_native_terminal_materialization_binding_v1",
    )
    for facade_name in ("zk_ams_facade", "vega_facade"):
        for marker in public_surface:
            require(
                sources[facade_name],
                marker,
                f"{facade_name} no longer propagates receipt consumer {marker}",
            )


class ReceiptConsumerSourceContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.sources = load_sources()

    def test_live_source_contract(self) -> None:
        validate_contract(self.sources)

    def assert_mutation_rejected(self, name: str, old: str, new: str) -> None:
        mutated = dict(self.sources)
        if old not in mutated[name]:
            self.fail(f"mutation preimage missing in {name}: {old!r}")
        mutated[name] = mutated[name].replace(old, new, 1)
        with self.assertRaises(AssertionError):
            validate_contract(mutated)

    def test_mutations_fail_closed(self) -> None:
        mutations = (
            ("consumer", "struct SplitDecryptionUseSealV1;", "pub struct SplitDecryptionUseSealV1;"),
            ("consumer", f"pub struct {SPLIT} {{", f"#[derive(Clone)]\npub struct {SPLIT} {{"),
            ("consumer", f"receipt: {RECEIPT},", f"receipt: &{RECEIPT},"),
            (
                "consumer",
                "receipt\n        .validate_v1()",
                "Ok::<(), ZkAmsMkheErrorV1>(())",
            ),
            (
                "decryption",
                "rns_link_use.consume_for_split_decryption_v1(statement, party_index)?;",
                "// receipt consumption removed",
            ),
            (
                "terminal",
                "rns_link_use.consume_for_terminal_materialization_v1(context, &materialized)?;",
                "// receipt consumption removed",
            ),
            (
                "composite",
                "ZkAmsMkheRnsNativeCompositeVerificationErrorV1::StageUnavailable(\n"
                "            ZkAmsMkheRnsNativeVerificationStageV1::TerminalHyraxBpBridge,",
                "ZkAmsMkheRnsNativeCompositeVerificationErrorV1::StageRejected(\n"
                "            ZkAmsMkheRnsNativeVerificationStageV1::TerminalHyraxBpBridge,",
            ),
            (
                "composite",
                "pub fn verify_zk_ams_mkhe_rns_native_composite_v1(\n"
                f"    transport: {TRANSPORT},",
                "pub fn verify_zk_ams_mkhe_rns_native_composite_v1(\n"
                "    envelope: ZkAmsMkheRnsNativeProofEnvelopeV1,",
            ),
            (
                "audit",
                "rns_link_transport_bound: true",
                "rns_link_transport_bound: false",
            ),
            (
                "audit",
                "terminal_materialization_receipt_enforced: true",
                "terminal_materialization_receipt_enforced: false",
            ),
            (
                "audit",
                "const _: () = assert!(CURRENT_OPEN_RECEIPT_CAPABILITY_BLOCKERS_V1 == 0x1f0);",
                "const _: () = assert!(CURRENT_OPEN_RECEIPT_CAPABILITY_BLOCKERS_V1 == 0);",
            ),
            (
                "audit",
                "const _: () = assert!(CURRENT_MISSING_IMPLEMENTATION_PREREQUISITES_V1 == 0x7c);",
                "const _: () = assert!(CURRENT_MISSING_IMPLEMENTATION_PREREQUISITES_V1 == 0);",
            ),
            (
                "audit",
                "self.validate_current_implementation_v1()",
                "Ok(())",
            ),
            ("audit", "release_available: false", "release_available: true"),
            (
                "zk_ams_facade",
                "bind_zk_ams_mkhe_rns_native_split_decryption_uses_v1",
                "removed_split_decryption_receipt_binder",
            ),
        )
        for name, old, new in mutations:
            with self.subTest(name=name, old=old):
                self.assert_mutation_rejected(name, old, new)


if __name__ == "__main__":
    unittest.main()
