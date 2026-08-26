"""Source contract for the runtime-provider broker backend setters."""

from __future__ import annotations

import hashlib
import json
import re
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
API_PATH = REPO_ROOT / "crates" / "irohad" / "src" / "runtime_provider_broker" / "api.rs"
MACRO_NAME = "define_runtime_provider_backends_v1"
INVOCATION_MARKER = f"{MACRO_NAME}! {{"
EXPECTED_METHODS = (
    "with_bootle_lantern_issuance",
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
    "with_appeal_finance_transaction_signer",
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
    "with_moderation_panel_notification_archive",
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
    "with_evidence_viewer_transparency_publisher",
    "with_soracloud_runtime_mutation_signer",
)
EXPECTED_INVENTORY_SHA256 = (
    "af516aadd86f91d903592d4b10e115facaae321752203529458100cba397b53a"
)
EXPECTED_TEMPLATE_SHA256 = (
    "e3f801cc46ad8482d68c4b4b7bf57b65f134c59cf5d385f15f99cc48ed831201"
)
ENTRY_PATTERN = re.compile(
    r"(?P<docs>(?:        ///[^\n]*\n)+)"
    r"        (?P<kind>optional|repeated) (?P<field>[A-Za-z0-9_]+): "
    r"(?P<backend>[^\n]+?) => pub fn (?P<name>with_[A-Za-z0-9_]+)"
    r"\((?P<argument>[A-Za-z0-9_]+)\)"
    r"(?:, \"(?P<debug_label>[^\"]+)\")?;"
)


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def _balanced_braces(source: str, marker: str) -> tuple[int, int, str]:
    start = source.index(marker)
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


def _inventory(source: str) -> list[tuple[str, str, str, str, str, str, str]]:
    _start, _end, invocation = _balanced_braces(source, INVOCATION_MARKER)
    struct_open = invocation.index("{")
    struct_close = invocation.rfind("}")
    body = invocation[struct_open + 1 : struct_close]
    matches = list(ENTRY_PATTERN.finditer(body))
    _require(len(matches) == len(EXPECTED_METHODS), "backend entry count changed")
    residue = ENTRY_PATTERN.sub("", body)
    _require(not residue.strip(), "backend macro invocation contains unparsed source")
    records = []
    for match in matches:
        docs = "\n".join(
            line.strip() for line in match.group("docs").rstrip("\n").splitlines()
        )
        backend = "".join(match.group("backend").split()).replace(",>", ">")
        records.append(
            (
                match.group("kind"),
                docs,
                match.group("field"),
                backend,
                match.group("name"),
                match.group("argument"),
                match.group("debug_label") or "",
            )
        )
    return records


def _validate_source(source: str) -> None:
    records = _inventory(source)
    names = tuple(record[4] for record in records)
    _require(names == EXPECTED_METHODS, "backend setter inventory changed")
    repeated = [record for record in records if record[0] == "repeated"]
    _require(
        len(repeated) == 1 and repeated[0][4] == "with_appeal_finance_transaction_signer",
        "the exact repeated backend setter changed",
    )
    payload = json.dumps(records, ensure_ascii=False, separators=(",", ":")).encode()
    _require(
        hashlib.sha256(payload).hexdigest() == EXPECTED_INVENTORY_SHA256,
        "backend docs, kind, type, argument, order, or field mapping changed",
    )
    template_start, template_end, _ = _balanced_braces(
        source, "macro_rules! define_runtime_provider_backend_setter_v1"
    )
    template = "".join(source[template_start:template_end].split())
    _require(
        hashlib.sha256(template.encode()).hexdigest() == EXPECTED_TEMPLATE_SHA256,
        "backend setter visibility, must-use posture, assignment, or return value changed",
    )


class RuntimeProviderBrokerBackendSetterSourceTests(unittest.TestCase):
    def test_backend_setter_contract(self) -> None:
        _validate_source(API_PATH.read_text(encoding="utf-8"))

    def test_contract_rejects_source_mutations(self) -> None:
        source = API_PATH.read_text(encoding="utf-8")
        first_entry = ENTRY_PATTERN.search(source)
        self.assertIsNotNone(first_entry)
        assert first_entry is not None
        mutations = {
            "missing entry": source[: first_entry.start()] + source[first_entry.end() :],
            "wrong kind": source.replace(
                "optional bootle_lantern_issuance:",
                "repeated bootle_lantern_issuance:",
                1,
            ),
            "wrong field": source.replace(
                "optional bootle_lantern_issuance:",
                "optional privacy_cycle_prf_provider:",
                1,
            ),
            "changed rustdoc": source.replace(
                "native Bootle/Lantern issuer", "mutated Bootle/Lantern issuer", 1
            ),
            "changed signature": source.replace(
                "optional bootle_lantern_issuance: Arc<dyn BootleLanternIssuanceBrokerBackendV1>",
                "optional bootle_lantern_issuance: Arc<dyn sorafs_node::GovernanceDagRuntimeSigner>",
                1,
            ),
            "narrowed visibility": source.replace(
                "pub fn $method(mut self", "pub(crate) fn $method(mut self", 1
            ),
            "lost must-use": source.replace(
                "        #[must_use]\n        pub fn $method(mut self, $argument: $backend)",
                "        pub fn $method(mut self, $argument: $backend)",
                1,
            ),
            "lost optional assignment": source.replace("Some($argument)", "$argument", 1),
            "changed repeated assignment": source.replace(
                "self.$field.push($argument);", "self.$field = vec![$argument];", 1
            ),
        }
        for label, mutation in mutations.items():
            with self.subTest(label=label):
                with self.assertRaises((AssertionError, ValueError)):
                    _validate_source(mutation)


if __name__ == "__main__":
    unittest.main()
