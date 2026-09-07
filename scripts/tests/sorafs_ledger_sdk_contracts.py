"""Source and documentation contracts for canonical native SoraFS ledger SDKs.

The collected rollout tests invoke these assertions directly. Keep complete
route, transport, typed-query, and open deployment criteria here so the parent
inventory stays within its source budget without weakening the checks.
"""

from pathlib import Path
import re

from scripts.tests.sorafs_rollout_gate_source_support import read_source as read

REPO_ROOT = Path(__file__).resolve().parents[2]
IROHA_CLIENT_RS = REPO_ROOT / "crates" / "iroha" / "src" / "client.rs"
SORAFS_REPAIR_PLAN = REPO_ROOT / "specs" / "sorafs_repair_plan.md"
SORAFS_RESERVE_RENT_PLAN = REPO_ROOT / "specs" / "sorafs_reserve_rent_plan.md"
SORAFS_REPUTATION_PLAN = REPO_ROOT / "specs" / "sorafs_reputation_plan.md"


def assert_repair_chain_authority_is_closed_and_live_evidence_stays_open_in_docs() -> None:
    source = read(SORAFS_REPAIR_PLAN)
    normalized = re.sub(r"\s+", " ", source)

    required_open = (
        "Every command route accepts exactly one caller-signed `SignedTransaction` containing the route-specific native repair instruction and forwards it through strict durable transaction ingress. Reads return finalized ledger projections; obsolete local status-by-manifest, SSE, and WebSocket authority routes are not shipped.",
        "The former local `RepairManager`, `FileRepairStore`, repair checkpoint, mutation/event history, scheduler, and compatibility APIs have been deleted.",
        "The storage executor accepts only a fully validated native task read at an exact finalized cursor and requires the current lease owner, generation, revision, provider binding, and expiry before any storage I/O.",
        "GC and reconciliation consume one complete, bounded task projection collected from a single immutable finalized query view; a truncated, drifting, malformed, or unbound projection fails closed.",
        "Use the rollout gate only after exact-live-lease execution and restart reconciliation have been proved in the reviewed deployment",
        "The checker recognizes `sorafs.repair.*` SF-8b rollout schemas for auditor roster, failure capture, signed auditor API, worker lifecycle, event streams, governance handoff, observability, and governance approval evidence.",
        "raw PoR/PoTR evidence, raw repair payloads, signed auditor requests, response bodies, signed transactions, secrets, and ledgers are absent",
        "matches a valid auditor-roster artifact, and worker lifecycle / event stream / governance handoff artifacts carry an `evidence_bundle_digest_hex` that matches a valid PoR/PoTR failure-capture artifact",
        "governance approval artifacts carry a `handoff_digest_hex` that matches a valid governance handoff artifact",
        "The SF-8b rollout evidence gate, collection planner, operator argfile templates, and focused tests are implemented for payload-free deployed evidence review",
        "The competing local repair authority and GC/reconciliation checkpoint dependencies are removed.",
        "Remaining rollout work is genuine four-validator evidence for a production PoR/PoTR failure, one cross-peer lease and terminal outcome, escalation/appeal, restart reconciliation, and governance handoff, followed by the SF-8b rollout evidence gate.",
    )
    assert [phrase for phrase in required_open if phrase not in normalized] == []
    client = read(IROHA_CLIENT_RS) + read(IROHA_CLIENT_RS.parent / "client" / "repair.rs")
    # The shared submitter validates the exact route before admission or HTTP.
    compact_client = re.sub(r"\s+", "", read(IROHA_CLIENT_RS))
    assert "sorafs_transaction_submitter!(post_sorafs_repair_transaction(route:SorafsRepairCommandRoute),repair::validate_transaction_route," in compact_client
    submitter = compact_client.split("macro_rules!sorafs_transaction_submitter", 1)[1].split("macro_rules!sorafs_signed_json_methods", 1)[0]
    assert submitter.index("$validate($route,transaction)?;") < submitter.index("self.ensure_transaction_submit_compatibility()?;") < submitter.index(".send_blocking()")
    # Consolidated native tests must remain registered and invoke the complete helpers.
    for contract in ('route', 'read_success', 'transport'):
        assert re.search(
            rf"#\[test\]\s*fn repair_{contract}_contract\(\)\s*\{{\s*assert_repair_{contract}_contract\(\);\s*\}}",
            client,
        )
    shared_page_validation = read(IROHA_CLIENT_RS.parent / "client" / "reserve.rs")
    assert [
        marker
        for marker in (
            'mod repair;',
            'repair::validate_transaction_route,',
            'response.status() != StatusCode::OK',
            'get_all("content-type")',
            'Some(APPLICATION_JSON)',
            'wrapper.len() != 2',
            'Some("finalized_chain")',
            'RepairFinalizedStatusV1',
            'RepairLedgerTaskPageV1',
            'RepairFinalizedTaskV1',
            'RepairFinalizedEventPageV1',
            'validate_finalized_cursor',
            'validate_event_successor',
            'previous.event_index.checked_add(1)',
            'super::reserve::validate_id_page(',
            'page.has_more != page.next_after.is_some()',
            'REPAIR_DEFAULT_PAGE_LIMIT_V1: u32 = 50',
            'limit.unwrap_or(REPAIR_DEFAULT_PAGE_LIMIT_V1)',
            'repair::validate_status_response(response, finalized)',
            'repair::validate_tasks_response(response, filter)',
            'repair::validate_task_response(response, &ticket_id.0, finalized)',
            'repair::validate_events_response(response, filter)',
            'fn assert_repair_route_contract()',
            'for (route, instruction) in exact_route_instructions()',
            'assert_rejected_before_http(&client, SorafsRepairCommandRoute::Appeal, &report)',
            'assert_rejected_before_http(&client, SorafsRepairCommandRoute::Heartbeat, &claim)',
            'Executable::Instructions(vec![report.clone(), report].into())',
            'Executable::Ivm(IvmBytecode::from_compiled(vec![0x00]))',
            'fn assert_repair_read_success_contract()',
            'repair_read_response_binding_rejects_wrapper_finality_and_ticket_mismatches',
            'repair_task_page_response_binding_rejects_bounds_order_and_bad_continuations',
            'repair_task_page_response_binding_rejects_omitted_limit_over_torii_default',
            'repair_event_page_response_binding_rejects_bounds_order_and_bad_continuations',
            'repair_event_page_response_binding_rejects_omitted_limit_over_torii_default',
            'repair_event_page_response_binding_rejects_noncanonical_block_index_successors',
            'fn assert_repair_transport_contract()',
            'assert_non_ok_preserved(&response.expect("non-OK repair response"))',
            'assert_rejected(&response)',
            'assert_eq!(snapshots.lock().expect("snapshot lock").len(), 4)',
        )
        if marker not in client
    ] == []
    assert [
        marker
        for marker in (
            "pub(super) fn validate_id_page",
            "if has_more != next_after.is_some()",
            "records.last().map(&id) != Some(next)",
        )
        if marker not in shared_page_validation
    ] == []


def assert_reserve_rent_chain_authoritative_contract_stays_open_until_evidence() -> None:
    source = read(SORAFS_RESERVE_RENT_PLAN)
    normalized = re.sub(r"\s+", " ", source)

    required_contract = (
        "SoraFS V1 treats the native reserve ledger as the only authority",
        "they do not own an independent reserve balance or lifecycle state",
        "the supervised Torii reserve worker",
        "Pre-release reserve state encoded without the V1 settlement anchor is not compatible.",
        "Validator or Torii wall clocks never participate.",
        "The former process-local reserve lifecycle scheduler, lifecycle/movement routes, local reserve checkpoint, and CLI adapters are removed from the V1 surface.",
        "Production consumers use finalized typed queries",
        "Reputation, orderbook, compliance, and transparency consumers must use these committed projections",
        "The reserve lane is release-ready only when these tests, the full workspace and SDK gates, the four-validator deployment exercise, security review, disaster recovery rehearsal, and signed aggregate readiness evidence all pass.",
    )
    assert [phrase for phrase in required_contract if phrase not in normalized] == []
    client = read(IROHA_CLIENT_RS) + read(IROHA_CLIENT_RS.parent / "client" / "reserve.rs") + read(IROHA_CLIENT_RS.parent / "http_default.rs")
    # The shared submitter validates the exact route before admission or HTTP.
    compact_client = re.sub(r"\s+", "", read(IROHA_CLIENT_RS))
    assert "sorafs_transaction_submitter!(post_sorafs_reserve_transaction(route:SorafsReserveCommandRoute),reserve::validate_transaction_route," in compact_client
    submitter = compact_client.split("macro_rules!sorafs_transaction_submitter", 1)[1].split("macro_rules!sorafs_signed_json_methods", 1)[0]
    assert submitter.index("$validate($route,transaction)?;") < submitter.index("self.ensure_transaction_submit_compatibility()?;") < submitter.index(".send_blocking()")
    # Consolidated native tests must remain registered and invoke the complete helpers.
    for contract in ('route', 'request'):
        assert re.search(
            rf"#\[test\]\s*fn reserve_{contract}_contract\(\)\s*\{{\s*assert_reserve_{contract}_contract\(\);\s*\}}",
            client,
        )
    assert [
        marker
        for marker in (
            'mod reserve;',
            'reserve::validate_transaction_route,',
            'finalized_json_request(',
            '"Accept-Encoding", "identity"',
            '.max_response_bytes(RESERVE_JSON_RESPONSE_MAX_BYTES_V1)',
            'fn assert_reserve_route_contract()',
            'for (route, instruction) in exact_route_instructions()',
            'assert_rejected_before_http(&client, route, &transaction)',
            'SorafsReserveCommandRoute::MovementDecision([0x71; 32])',
            'SorafsReserveCommandRoute::AppealDecision([0x72; 32])',
            'Executable::Instructions(vec![top_up.clone(), top_up].into())',
            'Executable::Ivm(IvmBytecode::from_compiled(vec![0x00]))',
            'reserve_read_response_binding_accepts_exact_typed_records_and_pages',
            'reserve_event_response_binding_accepts_exact_successors',
            'reserve_page_response_binding_separates_json_transport_and_norito_bounds',
            'reserve_read_response_binding_rejects_media_wrapper_finality_and_detail_mismatch',
            'reserve_read_response_binding_rejects_typed_semantic_mutants',
            'reserve_page_response_binding_rejects_bounds_order_exclusivity_and_continuation',
            'reserve_event_response_binding_rejects_gaps_finality_and_bad_continuations',
            'fn assert_reserve_request_contract()',
            'assert!(snapshots.lock().expect("snapshot lock").is_empty())',
            'assert_eq!(response.status(), StatusCode::CONFLICT)',
            'assert_eq!(response.body(), &[0x00, 0xFF, 0x51, 0x00])',
            'assert_eq!(snapshots.len(), paths.len())',
            'assert_exact_read_request(snapshot, &path)',
            'owned_http_client_does_not_follow_signed_body_redirects',
            'owned_async_http_client_does_not_follow_signed_body_redirects',
        )
        if marker not in client
    ] == []


def assert_reputation_docs_track_projector_hard_cut_and_remaining_runtime_work() -> None:
    source = read(SORAFS_REPUTATION_PLAN)
    normalized = re.sub(r"\s+", " ", source)
    assert "mod reputation_journal;" in read(IROHA_CLIENT_RS)
    reputation_client = read(IROHA_CLIENT_RS.parent / "client" / "reputation_journal.rs")
    assert "try_build_sorafs_reputation_journal_" not in reputation_client
    assert all(marker in reputation_client for marker in (
        "AccountTransactionDraft::new(", "SetSorafsReputationJournalAuthorityPolicy::new(",
        "AppendSorafsPorReputationJournalEntry::new(",
        "AppendSorafsStreamTokenReputationJournalEntry::new(",
    ))
    assert ".prepare_transaction(AccountTransactionDraft::new(" in reputation_client
    assert "account.sign_transaction(payload)" in re.sub(r"\s+", "", reputation_client)

    assert reputation_client.count("pub fn query_sorafs_reputation_journal_") == 3
    assert [
        marker
        for marker in (
            'pub fn query_sorafs_reputation_journal_authority_policy(',
            'pub fn query_sorafs_reputation_journal_event_by_source_id(',
            'pub fn query_sorafs_reputation_journal_events(',
            'canonical_por.source_kind()',
            'canonical_token.source_kind()',
            'assert_ne!(wrong_authority.recorded_by, client.account)',
            'event.validate(response_cursor)',
            'page.events.len() > usize::try_from(limit)',
            'after.is_none() && page.events.first().is_some_and(|event| event.sequence != 1)',
            'page.validate_after(after)',
            'canonical_drafts_sign_exact_typed_reputation_instructions',
            'reputation_inputs_expose_invalid_family_and_authority_before_drafting',
            'typed_queries_are_authenticated_and_preserve_exact_fields',
            'unpinned_source_query_rejects_malformed_event_response',
            'event_page_query_rejects_responses_outside_request_bounds',
            'query_validation_rejects_bad_inputs_without_http',
        )
        if marker not in reputation_client
    ] == []
    required_open = (
        "SFM-3 has two local foundations: the deterministic reputation V1 snapshot/proof core and a native committed input journal.",
        "This is source implementation, not a readiness claim.",
        "Torii's local-authoritative snapshot POST and the matching CLI publication command are removed. Latest, provider, weights, and event reads now consume only the fresh committed-derived projection after signed snapshot validation and authenticated Governance DAG readback. Snapshot-id reads resolve the exact authenticated snapshot from the durable immutable suffix capped at 1,024 entries and the publication-checkpoint byte ceiling; unknown or evicted ids return `404`.",
        "Capacity-dispute registration appends `Opened` atomically with the canonical dispute record, and `ResolveSorafsCapacityDispute` atomically updates that record and appends the exact revision-two `Resolved` event.",
        "The standard daemon owns the compact Kura-authenticated historical archive/query and captures it at the V2 commit boundary.",
        "Scoring engine (`reputation_engine`) | Aggregates finalized projections, runs the fixed-point EigenTrust-style algorithm, applies policy penalties, and generates canonical snapshot material. | Runs on the configured supervised interval and writes only the bounded durable checkpoint/outbox; publication becomes visible through the authenticated Governance DAG and committed-derived projection.",
        "Snapshot publisher (`reputation_publisher`) | Independently threshold-signs exact projector outbox material, publishes it to the Governance DAG/committed projection, and acknowledges the canonical result. | The supervised keyless worker is wired; production threshold-signer and authenticated DAG publication/readback adapters remain open.",
        "API gateway (`sorafs_reputation_api`) | Exposes authenticated read-only REST, SSE, and WebSocket committed projections. | The obsolete local POST is removed. Exact empty-body GETs require the signature quartet or exact witness. Latest/provider/weights/event reads use the ready committed projection; snapshot-id reads return the exact retained authenticated snapshot or `404` after bounded eviction, and the runtime cannot start in production until all required injected adapters exist.",
        "Strict non-secret `iroha_config` policy construction and the supervised finalized-query/threshold-signing/publication worker are implemented.",
        "The immutable historical query is no longer injectable: the daemon opens the explicitly bounded archive, performs zero-gap reconciliation against the authenticated Kura tip, preserves the activation floor when first enabled on a nonempty chain, and constructs the query from that archive.",
        "`IrohaRuntimeDeps` requires an externally authenticated journal-transaction submitter. The queue-backed validator-key submitter, the unsound current-head state adapter, and both fallbacks were removed.",
        "Missing, null/test-marked, or identity-substituted threshold-signer and Governance DAG adapters fail startup. The finalized query is daemon-owned and archive-backed rather than injectable.",
        "The threshold-signer boundary pins its production handle to the canonical trust-policy digest, which covers policy identity/version, quorum, the ordered Ed25519 public-key set, and revocations; startup and every signing call revalidate the binding before and after use, and every returned envelope is verified against the same policy.",
        "Governance DAG provider qualification now binds both the configured publisher peer identity and exact Ed25519 public key, so a same-key different-peer adapter fails before the reputation checkpoint opens.",
        "Daemon startup applies the runtime's complete exact-request bootstrap-view validator—anchor identity and chain/height, authority-policy activation time, canonical continuation, non-zero bounded request limit, and exact finalized cursor—before opening the reputation checkpoint.",
        "The same archive is threaded through Sumeragi and durably captures each fresh height after Kura finality and the WSV checkpoint but before `StateBlock::commit`; a capture failure requires committed recovery.",
        "Open under `V1-BLOCK-REPUTATION-RUNTIME-01`: `ReputationThresholdSignerClientV1` and `ReputationGovernanceDagClientV1` adapters; concrete stream-token callback-owner wiring; genuine qualification of the configured PoR replay archive and external threshold software-signing service; current DAG head/inclusion proof; integrated Rust validation; and reviewed four-peer rotation, recovery, retry, and failover evidence remain outstanding. No ledger page, credential, signature, or acknowledgement may be synthesized as a fallback.",
        "Production rollout:",
        "L1 remains open until the exact four-validator deployment supplies genuine immutable finalized-query, external threshold-signing, authenticated Governance DAG publication/readback/head-inclusion, PoR/token callback, restart/failover, live transport, and rollout evidence, including authenticated CLI collection or equivalent direct artifacts.",
        "Supply external threshold signer and authenticated Governance DAG publication/readback/head-inclusion adapters to the already-supervised runtime.",
        "Deploy the supervised publisher and API against the committed projection, exercise exact retained snapshot-id lookup and bounded eviction, and run four-peer end-to-end tests with orchestrator/indexer consumers.",
        "Capture live run evidence for snapshot freshness, ingest lag, low-score handling, SSE/WebSocket event delivery, and routing/incentive consumption",
        "Publish governance-approved weights with the governed `weights_digest_hex` carried by publish/latest rollout evidence, then archive the first production snapshot `.to`/JSON artifacts and proof replay evidence.",
        "Exercise rollback/stale-snapshot procedures before routing or incentives rely",
    )
    missing = [phrase for phrase in required_open if phrase not in normalized]
    stale_missing_adapter_claims = (
        "concrete production query and journal-delivery adapters remain open",
        "there is no non-test concrete implementation of those three adapters",
        "add the fixed-view active-policy reader and durable queue-backed",
        "deploy genuine finalized-query, threshold-signing",
        "remaining GET family still reads the old retained snapshot model",
        "standard-daemon committed read wiring remains open",
        "snapshot-id route remains latest-only",
        "snapshot-id route still retains only the latest snapshot",
        "Resolve the latest-only snapshot-id route",
        "`StateReputationFinalizedQueryV1`",
    )
    assert missing == []
    assert not any(phrase in normalized for phrase in stale_missing_adapter_claims)
