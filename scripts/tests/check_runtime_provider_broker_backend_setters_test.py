"""Source contract for the runtime-provider broker's optional backend setters."""

from __future__ import annotations

import hashlib
import json
import re
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
API_PATH = REPO_ROOT / "crates" / "irohad" / "src" / "runtime_provider_broker" / "api.rs"
MACRO_NAME = "define_optional_runtime_provider_backends_v1"
INVOCATION_MARKER = f"{MACRO_NAME}! {{"
PUSH_METHOD = "with_appeal_finance_transaction_signer"
EXPECTED_OPTIONAL_METHODS = (
    "with_bootle_lantern_issuance",
    "with_soracloud_runtime_mutation_signer",
    "with_soracloud_hf_inference_credential_provider",
    "with_moderation_quarantine_key_wrapper",
    "with_privacy_cycle_prf_provider",
    "with_privacy_release_anchor",
    "with_transparency_leader_lease_provider",
    "with_fenced_privacy_publisher",
    "with_fenced_privacy_head_reader",
    "with_governance_dag_signer",
    "with_governance_dag_ipfs_authenticator",
    "with_governance_dag_head_authenticator",
    "with_governance_dag_checkpoint_store",
    "with_stream_token_signer",
    "with_stream_token_gateway_admission",
    "with_appeal_finance_checkpoint",
    "with_proof_outcome_transaction_signer",
    "with_repair_transaction_signer",
    "with_reserve_transaction_signer",
    "with_orderbook_transaction_signer",
    "with_moderation_transaction_signer",
    "with_moderation_settlement_handoff",
    "with_moderation_publication_handoff",
    "with_moderation_panel_notification",
    "with_moderation_checkpoint_store",
    "with_provider_ingest_authenticated_source",
    "with_provider_ingest_signer_resolver",
    "with_provider_ingest_checkpoint_store",
    "with_provider_ingest_retention_authority",
    "with_reputation_finalized_archive_retention_authority",
    "with_reputation_journal_transaction_submitter",
    "with_reputation_journal_checkpoint",
    "with_reputation_threshold_signer",
    "with_reputation_governance_dag",
    "with_billing_finalized_query",
    "with_billing_journal_verifier",
    "with_billing_statement_signer",
    "with_billing_statement_publisher",
    "with_billing_acknowledgement_authority",
    "with_billing_epoch_witness_store",
    "with_pop_credential_provider_registry",
    "with_potr_gateway_signer",
    "with_potr_provider_signer",
    "with_gateway_acme_client",
    "with_gateway_compliance_feed_transport",
    "with_por_finalized_replay_archive",
    "with_evidence_viewer_webauthn",
    "with_evidence_viewer_grants",
    "with_evidence_viewer_receipt_signer",
    "with_evidence_viewer_erasure",
    "with_evidence_viewer_checkpoint_store",
    "with_evidence_viewer_compaction_archive",
    "with_moderation_panel_notification_archive",
    "with_evidence_viewer_transparency_publisher",
)
EXPECTED_INVENTORY_SHA256 = (
    "5f4b2adf9e9270f125ba22d6c5568ebf9c93b2c47b5f1406925d9acedae55124"
)
EXPECTED_TEMPLATE_SHA256 = (
    "4ea35e242e79986df77e5be281367e051524dc02e26af501fc76f1779c4355fb"
)
EXPECTED_PUSH_SHA256 = "090e663e9904afa474e4cfe2c4145a32b1c6e79537b5e1210a354a2b2055bf9d"
ENTRY_PATTERN = re.compile(
    r"(?P<docs>(?:        ///[^\n]*\n)+)"
    r"        (?P<name>with_[A-Za-z0-9_]+)\(\n"
    r"(?P<parameter>.*?)\n"
    r"        \) => (?P<field>[A-Za-z0-9_]+);",
    re.DOTALL,
)


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def _balanced_braces(source: str, marker: str, offset: int = 0) -> tuple[int, int, str]:
    start = source.index(marker, offset)
    opening = source.index("{", start)
    depth = 0
    for index in range(opening, len(source)):
        if source[index] == "{":
            depth += 1
        elif source[index] == "}":
            depth -= 1
            if depth == 0:
                return start, index + 1, source[opening + 1 : index]
    raise AssertionError(f"unterminated brace block after {marker}")


def _invocation_blocks(source: str) -> list[tuple[int, int, str]]:
    blocks = []
    offset = 0
    while source.find(INVOCATION_MARKER, offset) >= 0:
        block = _balanced_braces(source, INVOCATION_MARKER, offset)
        blocks.append(block)
        offset = block[1]
    return blocks


def _inventory(source: str) -> tuple[list[tuple[str, str, str, str, str]], list[int]]:
    records = []
    block_sizes = []
    for _, _, body in _invocation_blocks(source):
        matches = list(ENTRY_PATTERN.finditer(body))
        residue = ENTRY_PATTERN.sub("", body)
        _require(not residue.strip(), "optional-backend macro contains unparsed source")
        block_sizes.append(len(matches))
        for match in matches:
            parameter = "".join(match.group("parameter").split())
            argument, backend_type = parameter.split(":", 1)
            backend_type = backend_type.rstrip(",").replace(",>", ">")
            docs = "\n".join(
                line.strip() for line in match.group("docs").rstrip("\n").splitlines()
            )
            records.append(
                (
                    docs,
                    match.group("name"),
                    argument,
                    backend_type,
                    match.group("field"),
                )
            )
    return records, block_sizes


def _push_method_hash(source: str) -> tuple[int, int, str]:
    signature = f"    pub fn {PUSH_METHOD}("
    method_start = source.index(signature)
    attribute_start = source.rfind("    #[must_use]", 0, method_start)
    item_start = attribute_start
    while item_start:
        previous = source.rfind("\n", 0, item_start - 1) + 1
        line = source[previous:item_start]
        if line.startswith("    ///") or not line.strip():
            item_start = previous
        else:
            break
    _, method_end, _ = _balanced_braces(source, signature)
    compact = "".join(source[item_start:method_end].split())
    return item_start, method_end, hashlib.sha256(compact.encode()).hexdigest()


def _validate_source(source: str) -> None:
    records, block_sizes = _inventory(source)
    _require(block_sizes == [15, 39], f"unexpected macro block sizes: {block_sizes}")
    names = tuple(record[1] for record in records)
    _require(names == EXPECTED_OPTIONAL_METHODS, "optional backend setter inventory changed")
    payload = json.dumps(records, ensure_ascii=False, separators=(",", ":")).encode()
    inventory_hash = hashlib.sha256(payload).hexdigest()
    _require(
        inventory_hash == EXPECTED_INVENTORY_SHA256,
        "setter docs, signature, argument, order, or field mapping changed",
    )

    template_start, template_end, _ = _balanced_braces(
        source, f"macro_rules! {MACRO_NAME}"
    )
    template = "".join(source[template_start:template_end].split())
    _require(
        hashlib.sha256(template.encode()).hexdigest() == EXPECTED_TEMPLATE_SHA256,
        "setter macro visibility, attributes, assignment, or return value changed",
    )

    push_start, _, push_hash = _push_method_hash(source)
    _require(push_hash == EXPECTED_PUSH_SHA256, "Vec-push builder changed or was absorbed")
    explicit_with_methods = re.findall(r"^    pub fn (with_[A-Za-z0-9_]+)\(", source, re.MULTILINE)
    _require(
        explicit_with_methods == [PUSH_METHOD],
        f"unexpected explicit with_* methods: {explicit_with_methods}",
    )
    blocks = _invocation_blocks(source)
    _require(
        blocks[0][1] < push_start < blocks[1][0],
        "Vec-push builder no longer separates the two exact setter groups",
    )
    for *_, field in records:
        _require(
            source.count(f"{field}: None,") == 1,
            f"optional backend field {field} is not initialized exactly once",
        )


class RuntimeProviderBrokerBackendSetterSourceTests(unittest.TestCase):
    def test_optional_backend_setter_contract(self) -> None:
        _validate_source(API_PATH.read_text(encoding="utf-8"))

    def test_contract_rejects_source_mutations(self) -> None:
        source = API_PATH.read_text(encoding="utf-8")
        first_entry = ENTRY_PATTERN.search(source)
        self.assertIsNotNone(first_entry)
        assert first_entry is not None
        mutations = {
            "missing entry": source[: first_entry.start()] + source[first_entry.end() :],
            "wrong field": source.replace(
                ") => bootle_lantern_issuance;",
                ") => privacy_cycle_prf_provider;",
                1,
            ),
            "changed rustdoc": source.replace(
                "native Bootle/Lantern issuer", "mutated Bootle/Lantern issuer", 1
            ),
            "changed signature": source.replace(
                "backend: Arc<dyn BootleLanternIssuanceBrokerBackendV1>,",
                "backend: Arc<dyn sorafs_node::GovernanceDagRuntimeSigner>,",
                1,
            ),
            "narrowed visibility": source.replace(
                "pub fn $name(mut self", "pub(crate) fn $name(mut self", 1
            ),
            "lost must-use": source.replace(
                "            #[must_use]\n            pub fn $name",
                "            pub fn $name",
                1,
            ),
            "lost optional assignment": source.replace(
                "Some($argument)", "$argument", 1
            ),
            "changed Vec-push builder": source.replace(
                "self.appeal_finance_transaction_signers.push(signer);",
                "self.appeal_finance_transaction_signers = vec![signer];",
                1,
            ),
        }
        for label, mutation in mutations.items():
            with self.subTest(label=label):
                with self.assertRaises((AssertionError, ValueError)):
                    _validate_source(mutation)


if __name__ == "__main__":
    unittest.main()
