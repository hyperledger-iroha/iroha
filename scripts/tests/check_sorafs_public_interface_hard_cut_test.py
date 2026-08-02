"""Static guards for the SoraFS V1 public-interface hard cut."""

import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


def test_por_tree_exposes_only_fallible_proof_construction() -> None:
    """The retired migration alias must not re-enter the public proof API."""

    source = (ROOT / "crates/sorafs_car/src/lib.rs").read_text(encoding="utf-8")

    assert "pub fn try_prove_leaf(" in source
    assert "pub fn prove_leaf(" not in source


def test_reputation_committed_reads_have_no_projection_fallbacks() -> None:
    """Exact snapshot and event reads remain mandatory implementation hooks."""

    source = (
        ROOT / "crates/sorafs_node/src/reputation/runtime.rs"
    ).read_text(encoding="utf-8")
    trait_body = source.split("pub trait ReputationCommittedReadApiV1", 1)[1].split(
        "/// Activation state", 1
    )[0]

    assert re.search(
        r"fn committed_snapshot_by_id\(.*?\)\s*"
        r"-> Result<Option<ReputationSnapshotV1>, ReputationRuntimeError>;",
        trait_body,
        re.DOTALL,
    )
    assert re.search(
        r"fn committed_events_after\(.*?\)\s*"
        r"-> Result<Vec<ReputationSnapshotEventV1>, ReputationRuntimeError>;",
        trait_body,
        re.DOTALL,
    )
    assert "committed_read_projection()?" not in trait_body


def test_payload_sources_cannot_skip_exact_length_validation() -> None:
    """Random-access payload sources must not inherit a no-op length check."""

    source = (ROOT / "crates/sorafs_car/src/lib.rs").read_text(encoding="utf-8")
    trait_body = source.split("pub trait PayloadSource", 1)[1].split(
        "/// Streaming payload", 1
    )[0]

    assert re.search(
        r"fn ensure_exhausted\(&mut self, expected_len: u64\)\s*"
        r"-> Result<\(\), ChunkStoreError>;",
        trait_body,
        re.DOTALL,
    )
    assert "Ok(())" not in trait_body


def test_reference_validator_exposes_only_exact_v1_selectors() -> None:
    """The reference CLI must not retain command, kind, profile, or flag aliases."""

    source = (
        ROOT / "crates/sorafs_manifest/src/bin/sorafs-validate.rs"
    ).read_text(encoding="utf-8")
    production = source.split("#[cfg(test)]", 1)[0]

    assert '"hedging" | "billing"' not in production
    assert 'strip_prefix("--envelope=")' not in production
    for parser, next_parser, aliases in [
        ("fn parse_profile", "fn parse_repair_kind", ["cold"]),
        (
            "fn parse_repair_kind",
            "fn parse_pop_kind",
            ["task-record", "repair-task", "repair-evidence", "slash"],
        ),
        (
            "fn parse_pop_kind",
            "fn parse_hedging_kind",
            ["pop-credential", "root", "revocations", "issued-bundle"],
        ),
        (
            "fn parse_hedging_kind",
            "fn parse_orderbook_kind",
            ["feed", "decision", "line-item", "statement"],
        ),
        (
            "fn parse_orderbook_kind",
            "fn parse_orderbook_sign_kind",
            ["request", "cancel-request", "trade", "channel", "receipt"],
        ),
        (
            "fn parse_sign_kind",
            "fn read_signing_seed",
            [
                "provider-advert",
                "replication-order",
                "orderbook-payload",
                "governance-log-node",
            ],
        ),
    ]:
        body = production.split(parser, 1)[1].split(next_parser, 1)[0]
        for alias in aliases:
            assert f'"{alias}"' not in body

    for parser_name in ["PopArgs", "RepairArgs", "HedgingArgs", "OrderbookArgs"]:
        parser_body = production.split(f"impl {parser_name}", 1)[1].split(
            "#[derive(Debug, Default)]", 1
        )[0]
        assert "set_payload" not in parser_body


def test_transparency_source_selector_has_no_normalization_aliases() -> None:
    """Torii accepts only the seven documented transparency source path labels."""

    source = (ROOT / "crates/iroha_torii/src/sorafs/api.rs").read_text(
        encoding="utf-8"
    )
    body = source.split("fn parse_transparency_source_kind", 1)[1].split(
        "fn transparency_source_entry_from_body", 1
    )[0]

    assert ".trim()" not in body
    assert "to_ascii_lowercase" not in body
    assert "replace('_', \"-\")" not in body
    for alias in [
        "gar-receipt",
        "moderation-ballot",
        "appeal-finance-settlement",
        "legal-hold",
        "redaction",
        "evidence-viewer-access",
    ]:
        assert f'"{alias}"' not in body


def test_appeal_verdict_parser_has_one_exact_v1_spelling_per_value() -> None:
    """Appeal verdict parsing must not normalize or map compatibility spellings."""

    source = (ROOT / "crates/sorafs_orchestrator/src/appeals.rs").read_text(
        encoding="utf-8"
    )
    body = source.split("impl FromStr for AppealVerdict", 1)[1].split(
        "/// Mapping of refund/slash ratios", 1
    )[0]

    assert ".trim()" not in body
    assert "to_ascii_lowercase" not in body
    for alias in [
        "withdrawn-before-panel",
        "withdrawn_pre",
        "withdrawn-post",
        "pending",
    ]:
        assert f'"{alias}"' not in body

    cli = (
        ROOT / "crates/sorafs_orchestrator/src/bin/sorafs_cli.rs"
    ).read_text(encoding="utf-8")
    cli_body = cli.split("fn parse_appeal_verdict", 1)[1].split(
        "fn main()", 1
    )[0]
    assert "parse::<AppealVerdict>()" in cli_body
    assert "to_ascii_lowercase" not in cli_body


def test_javascript_sorafs_reference_inputs_are_camel_case_only() -> None:
    """JS runtime and TypeScript declarations expose one canonical input shape."""

    source = (ROOT / "javascript/iroha_js/src/sorafs.js").read_text(
        encoding="utf-8"
    )
    declarations = (ROOT / "javascript/iroha_js/index.d.ts").read_text(
        encoding="utf-8"
    )
    reference_source = source.split("export function validateOrderbookPayload", 1)[
        1
    ].split("function assertNonEmptyString", 1)[0]
    declarations = declarations.split("export interface SorafsOrderbookValidationOptions", 1)[
        1
    ].split("export function decodeReplicationOrder", 1)[0]

    for alias in [
        "generated_at",
        "commitment_label",
        "challenge_label",
        "proof_label",
        "now_unix",
        "expected_node_cid",
        "expected_block_cid",
        "head_label",
        "noritoBytes",
        "norito_bytes",
    ]:
        assert alias not in reference_source
        assert alias not in declarations
    assert "payload?: SorafsReferenceBytesInput" not in declarations
    assert "bytes: SorafsReferenceBytesInput" in declarations
    assert "rejectUnexpectedFields" in reference_source
