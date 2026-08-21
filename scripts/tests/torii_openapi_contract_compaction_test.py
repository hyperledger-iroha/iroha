"""Fail-closed source guard for the typed Torii OpenAPI contract compaction."""

from __future__ import annotations

import copy
import hashlib
import hmac
import json
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
FINALITY = ROOT / "crates/iroha_torii/src/openapi/tests/finality_app_contracts.rs"
SORAFS = ROOT / "crates/iroha_torii/src/openapi/tests/sorafs_contracts.rs"
ASSET = ROOT / "crates/iroha_torii/src/openapi/tests/openapi_contracts_v1.json"
ASSET_VERSION = 1
BASELINE_RUST_LINES = 4_902
MAX_POSTIMAGE_RUST_LINES = 1_902
MINIMUM_NET_REDUCTION = 3_000
SECTION_ORDER = (
    "evidence.audit.description",
    "evidence.audit.success",
    "evidence.schemas",
    "pin.register.description",
    "pin.list.description",
    "pin.manifest.description",
    "pin.manifest.retired",
    "replication.description",
    "proof.por.required",
    "proof.pdp.failures",
    "proof.potr.failures",
    "sumeragi.da.required",
    "bridge.proof.required",
    "bridge.attestation.required",
    "finality.artifact.required",
    "height.context.required",
    "height.context.optional",
    "validator.power.required",
    "dual.quorum.required",
    "block.subject.required",
    "block.subject.optional",
    "merge.carrier.required",
    "execution.required",
    "execution.optional",
    "qc.required",
    "snapshot.bootstrap.required",
    "next.epoch.required",
    "bridge.commitment.required",
    "bridge.bundle.required",
    "block.header.required",
    "block.header.optional",
    "ledger.state_finality.required",
    "ledger.state_finality.retired",
    "ledger.state_finality.retired_paths",
    "ledger.state_finality.retired_schemas",
    "bridge.components",
    "bridge.retired",
    "fixture.header.required",
    "fixture.artifact.fields",
    "fixture.execution.fields",
    "fixture.retired",
    "lifecycle.required",
    "status.present",
    "status.absent",
    "native.receipt.required",
    "native.leg.required",
    "native.proposal.required",
    "native.body.required",
    "hf.headers",
    "private.receipt.metadata",
    "app.page.required",
    "app.page.properties",
    "repo.agreement.fields",
    "repo.query.fields",
    "contract.alias.request.required",
    "contract.alias.binding.required",
    "contract.alias.binding.optional",
    "contract.alias.response.required",
    "governed.found.fields",
    "governed.missing.fields",
)
FINALITY_TESTS = (
    "sumeragi_v2_da_schema_requires_reed_solomon16_without_plain_compatibility",
    "bridge_finality_v2_schemas_are_exact_closed_and_bounded",
    "bridge_finality_schema_matches_norito_json_and_decoder_rejects_v1_fields",
    "ledger_state_endpoints_expose_one_closed_authenticated_v2_schema",
    "bridge_finality_operations_describe_durable_v2_evidence",
    "generated_spec_documents_read_only_nexus_lifecycle_status",
    "generated_spec_documents_exact_authoritative_sumeragi_v2_status",
    "generated_spec_documents_soracloud_private_uploaded_model_routes",
    "generated_spec_documents_app_query_page_metadata",
    "alias_openapi_documents_optional_public_and_exact_restricted_auth",
    "protected_contract_identity_openapi_is_signed_and_exact",
    "multisig_read_auth_contract_is_path_specific",
)
SORAFS_TESTS = (
    "evidence_audit_openapi_requires_and_returns_exact_cursors",
    "evidence_openapi_matches_authenticated_protocol_contract",
    "sorafs_pin_register_openapi_is_caller_signed_transaction_transport",
    "sorafs_storage_token_openapi_requires_operator_and_diagnostic_headers",
    "sorafs_storage_and_inventory_openapi_matches_authenticated_catalog",
    "sorafs_pin_list_openapi_is_finalized_bounded_keyset_readback",
    "sorafs_pin_manifest_openapi_is_finalized_native_readback",
    "sorafs_replication_openapi_is_a_strict_chain_authoritative_v1_projection",
    "moderation_dead_letter_openapi_is_typed_bounded_and_dual_control",
    "hedging_billing_openapi_is_authenticated_bounded_and_private",
    "proof_stream_openapi_matches_the_closed_canonical_envelope",
)


class ContractAssetError(ValueError):
    """Raised when an asset does not satisfy the pinned V1 envelope."""


def _load_asset(payload: bytes, pinned_length: int, pinned_digest: str) -> dict[str, list[str]]:
    if len(payload) != pinned_length:
        raise ContractAssetError("asset length drift")
    digest = hashlib.sha256(payload).hexdigest()
    if not hmac.compare_digest(digest, pinned_digest):
        raise ContractAssetError("asset digest drift")
    try:
        root = json.loads(payload)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ContractAssetError("invalid JSON") from error
    if type(root) is not dict or set(root) != {"version", "sections"}:
        raise ContractAssetError("asset root shape")
    if type(root["version"]) is not int or root["version"] != ASSET_VERSION:
        raise ContractAssetError("asset version")
    sections = root["sections"]
    if type(sections) is not list or len(sections) != len(SECTION_ORDER):
        raise ContractAssetError("section count")
    indexed: dict[str, list[str]] = {}
    for expected_id, section in zip(SECTION_ORDER, sections):
        if type(section) is not dict or set(section) != {"id", "values"}:
            raise ContractAssetError("section shape")
        if section["id"] != expected_id or section["id"] in indexed:
            raise ContractAssetError("section order")
        values = section["values"]
        if type(values) is not list or not values:
            raise ContractAssetError("empty inventory")
        if any(type(value) is not str or not value for value in values):
            raise ContractAssetError("non-string inventory")
        if len(set(values)) != len(values):
            raise ContractAssetError("duplicate inventory value")
        indexed[expected_id] = values
    return indexed


def _rust_pins(source: str) -> tuple[int, str, int, tuple[str, ...]]:
    length = int(re.search(r"OPENAPI_CONTRACT_ASSET_LEN: usize = ([\d_]+);", source).group(1).replace("_", ""))
    digest = re.search(r'OPENAPI_CONTRACT_ASSET_SHA256: &str =\s*"([0-9a-f]{64})";', source).group(1)
    version = int(re.search(r"OPENAPI_CONTRACT_ASSET_VERSION: u64 = (\d+);", source).group(1))
    order_block = re.search(r"OPENAPI_CONTRACT_SECTION_ORDER: &\[&str\] = &\[(.*?)\n\];", source, re.S).group(1)
    order = tuple(re.findall(r'^\s*"([a-z0-9_.]+)",\s*$', order_block, re.M))
    return length, digest, version, order


def _test_names(source: str) -> tuple[str, ...]:
    return tuple(re.findall(r"(?m)^#\[test\]\nfn ([a-z0-9_]+)\(\)", source))


class ToriiOpenapiContractCompactionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.finality = FINALITY.read_text(encoding="utf-8")
        self.sorafs = SORAFS.read_text(encoding="utf-8")
        self.payload = ASSET.read_bytes()
        self.length, self.digest, self.version, self.rust_order = _rust_pins(self.sorafs)

    def test_asset_is_exactly_pinned_and_all_inventories_are_consumed(self) -> None:
        self.assertEqual(self.version, ASSET_VERSION)
        self.assertEqual(self.rust_order, SECTION_ORDER)
        inventories = _load_asset(self.payload, self.length, self.digest)
        consumers = self.sorafs[self.sorafs.index("const OPENAPI_CONTRACT_ASSET:") :] + self.finality
        self.assertEqual(set(inventories), set(SECTION_ORDER))
        for section_id in SECTION_ORDER:
            self.assertIn(f'"{section_id}"', consumers)

    def test_version_length_digest_order_and_scalar_mutations_fail_closed(self) -> None:
        with self.assertRaises(ContractAssetError):
            _load_asset(self.payload + b"\n", self.length, self.digest)
        mutated_byte = bytearray(self.payload)
        mutated_byte[self.payload.index(b"genesis")] ^= 1
        with self.assertRaises(ContractAssetError):
            _load_asset(bytes(mutated_byte), self.length, self.digest)
        root = json.loads(self.payload)
        for mutation in ("version", "order", "scalar", "duplicate"):
            hostile = copy.deepcopy(root)
            if mutation == "version":
                hostile["version"] = 2
            elif mutation == "order":
                hostile["sections"][0], hostile["sections"][1] = hostile["sections"][1], hostile["sections"][0]
            elif mutation == "scalar":
                hostile["sections"][0]["values"][0] = {"body": "relocated"}
            else:
                hostile["sections"][0]["values"][1] = hostile["sections"][0]["values"][0]
            encoded = json.dumps(hostile, separators=(",", ":")).encode()
            with self.subTest(mutation=mutation), self.assertRaises(ContractAssetError):
                _load_asset(encoded, len(encoded), hashlib.sha256(encoded).hexdigest())

    def test_historical_test_inventory_and_typed_runner_architecture_are_frozen(self) -> None:
        self.assertEqual(_test_names(self.finality), FINALITY_TESTS)
        self.assertEqual(_test_names(self.sorafs), SORAFS_TESTS)
        combined = self.sorafs + self.finality
        for record in ("SchemaShape", "PropertyRefContract", "OperationResponseContract"):
            self.assertIn(f"struct {record}", combined)
        for forbidden in ("Box<dyn Fn", "impl Fn", "dyn Fn", "ActionContract", "BodyContract", "callback"):
            self.assertNotIn(forbidden, combined)
        self.assertNotIn("#[ignore]", combined)

    def test_rust_line_budget_is_a_real_whole_tranche_reduction(self) -> None:
        postimage = len(self.finality.splitlines()) + len(self.sorafs.splitlines())
        self.assertLessEqual(postimage, MAX_POSTIMAGE_RUST_LINES)
        self.assertGreaterEqual(BASELINE_RUST_LINES - postimage, MINIMUM_NET_REDUCTION)
        self.assertLessEqual(max(map(len, (self.finality + self.sorafs).splitlines())), 400)


if __name__ == "__main__":
    unittest.main()
