"""Static proof-stream assertions shared by rollout contract tests."""

from __future__ import annotations

import re
from collections.abc import Callable
from pathlib import Path


def assert_pdp_and_potr_proof_streams_use_exact_finalized_chain_projections(
    *,
    read: Callable[[Path], str],
    api_path: Path,
    openapi_path: Path,
) -> None:
    """Assert PDP and PoTR use exact finalized chain projections."""

    api = read(api_path)
    openapi = read(openapi_path)

    renderer_start = api.index("fn render_finalized_proof_stream_response(")
    renderer_end = api.index(
        "\npub(crate) async fn handle_post_sorafs_pdp_challenge(",
        renderer_start,
    )
    renderer = re.sub(r"\s+", " ", api[renderer_start:renderer_end])
    assert "outcome.manifest_digest.as_bytes() != &request.manifest_digest" in renderer
    assert "outcome.provider_id.as_bytes() != &request.provider_id" in renderer
    assert "request.challenge_id != Some(outcome.identity_digest)" in renderer
    assert "receipt.request_id != Some(expected_job_id)" in renderer
    assert "proof_outcome_request_mismatch_response(proof_kind_label)" in renderer

    pdp_dispatch_start = api.index("ProofStreamKind::Pdp => {")
    pdp_dispatch_end = api.index("\n    }\n}", pdp_dispatch_start)
    pdp_dispatch = re.sub(r"\s+", " ", api[pdp_dispatch_start:pdp_dispatch_end])
    assert ".expect(\"validation guarantees challenge id for PDP\")" in pdp_dispatch
    assert (
        "FindSorafsProofOutcome::new( ProofOutcomeKindV1::Pdp, challenge_id, expected_finalized_cursor, )"
        in pdp_dispatch
    )
    assert "ProofOutcomeFinalizedCursorV1" in pdp_dispatch
    assert "validated finalized cursor is complete" in pdp_dispatch
    assert "query.execute(&state.state.query_view())" in pdp_dispatch
    assert (
        "render_finalized_proof_stream_response(&state, &finalized, &request)"
        in pdp_dispatch
    )
    assert "challenge_status" not in pdp_dispatch

    potr_dispatch_start = api.index("ProofStreamKind::Potr => {")
    potr_dispatch_end = api.index("ProofStreamKind::Pdp => {", potr_dispatch_start)
    potr_dispatch = re.sub(r"\s+", " ", api[potr_dispatch_start:potr_dispatch_end])
    assert (
        ".expect(\"validation guarantees orchestrator job id for PoTR\")"
        in potr_dispatch
    )
    assert (
        "potr_request_scope_digest_v1(request.manifest_digest, provider_id, job_id)"
        in potr_dispatch
    )
    assert (
        "FindSorafsProofOutcome::new( ProofOutcomeKindV1::Potr, identity_digest, expected_finalized_cursor, )"
        in potr_dispatch
    )
    assert "ProofOutcomeFinalizedCursorV1" in potr_dispatch
    assert "validated finalized cursor is complete" in potr_dispatch
    assert "query.execute(&state.state.query_view())" in potr_dispatch
    assert (
        "render_finalized_proof_stream_response(&state, &finalized, &request)"
        in potr_dispatch
    )

    pdp_schema_start = openapi.index('"SorafsProofStreamPdpRequestV1".to_owned()')
    pdp_schema_end = openapi.index(
        '"SorafsProofStreamPotrRequestV1".to_owned()',
        pdp_schema_start,
    )
    pdp_schema = openapi[pdp_schema_start:pdp_schema_end]
    pdp_required_start = pdp_schema.index('"required": [')
    pdp_required_end = pdp_schema.index("],", pdp_required_start)
    pdp_required = pdp_schema[pdp_required_start:pdp_required_end]
    assert '"challenge_id_hex"' in pdp_required
    assert '"additionalProperties": false' in pdp_schema

    potr_schema_start = pdp_schema_end
    potr_schema_end = openapi.index(
        '"SorafsProofStreamHttpRequestV1".to_owned()',
        potr_schema_start,
    )
    potr_schema = openapi[potr_schema_start:potr_schema_end]
    potr_required_start = potr_schema.index('"required": [')
    potr_required_end = potr_schema.index("],", potr_required_start)
    potr_required = potr_schema[potr_required_start:potr_required_end]
    assert '"orchestrator_job_id_hex"' in potr_required
    assert '"additionalProperties": false' in potr_schema
    assert (
        "chain-authoritative request-scope identity with the manifest and provider"
        in potr_schema
    )

    assert "`proof_kind=pdp` is reserved for future SF-13 work" not in openapi
    assert "rejected as an unsupported proof kind" not in openapi


def assert_sorafs_cli_proof_stream_uses_authenticated_native_pin_projection(
    *,
    read: Callable[[Path], str],
    cli_path: Path,
) -> None:
    """Assert the CLI proof stream is authenticated and native-pin bound."""

    cli = read(cli_path)
    command_start = cli.index("fn proof_stream(raw_args: Vec<String>)")
    command_end = cli.index("\nfn write_proof_stream_evidence(", command_start)
    command = cli[command_start:command_end]
    endpoint_start = cli.index("fn proof_stream_endpoint(")
    endpoint_end = cli.index("\nfn proof_stream_pin_manifest_endpoint(", endpoint_start)
    endpoint_policy = cli[endpoint_start:endpoint_end]
    fetch_start = cli.index("fn fetch_finalized_pin_manifest(")
    fetch_end = cli.index("\nfn validate_finalized_pin_manifest(", fetch_start)
    native_fetch = cli[fetch_start:fetch_end]
    validation_start = fetch_end + 1
    validation_end = cli.index("\nfn payload_free_proof_stream_event(", validation_start)
    validation = cli[validation_start:validation_end]
    event_start = validation_end + 1
    event_end = cli.index("\nfn proof_stream(raw_args: Vec<String>)", event_start)
    event_projection = cli[event_start:event_end]

    assert 'const PROOF_STREAM_ROUTE_V1: &str = "/v1/sorafs/proof/stream";' in cli
    assert 'const PIN_MANIFEST_ROUTE_PREFIX_V1: &str = "/v1/sorafs/pin/";' in cli
    assert 'parsed.scheme() != "https"' in endpoint_policy
    for rejected_component in (
        "parsed.username().is_empty()",
        "parsed.password().is_some()",
        "parsed.query().is_some()",
        "parsed.fragment().is_some()",
    ):
        assert rejected_component in endpoint_policy
    assert "raw != canonical_endpoint" in endpoint_policy
    assert "raw != canonical_origin && raw != canonical_origin_with_slash" in endpoint_policy
    assert ".https_only(true)" in cli
    assert ".redirect(RedirectPolicy::none())" in cli
    assert ".no_proxy()" in cli
    assert native_fetch.count(".bearer_auth(bearer_token)") == 1
    assert command.count(".bearer_auth(&bearer_token)") == 1
    assert "fetch_finalized_pin_manifest(" in command
    assert "validate_finalized_pin_manifest(" in command
    assert "PinStatus::Approved(_)" in validation
    for manifest_binding in (
        "record.digest.as_bytes() != local_manifest_digest",
        "record.root_cid.as_bytes().as_slice() != local_manifest.root_cid.as_slice()",
        "record.chunker != chunker_handle_from_profile(&local_manifest.chunking)",
        "record.chunk_digest_sha3_256 != local_manifest.chunk_digest_sha3_256",
        "record.por_root != local_manifest.por_root",
        "record.content_length != local_manifest.content_length",
        "record.policy != convert_pin_policy(&local_manifest.pin_policy)",
        "finalized.finalized_cursor.height == 0",
        ".finalized_cursor\n        .block_hash",
    ):
        assert manifest_binding in validation
    assert "MAX_PROOF_STREAM_SAMPLE_COUNT" in command
    assert "read_file_bounded(&manifest_path, manifest_byte_limit, \"manifest\")" in command
    assert "expected_finalized_height: Some(validated_pin.finalized_height)" in command
    assert (
        "expected_finalized_block_hash: Some(validated_pin.finalized_block_hash)"
        in command
    )
    assert "ProofStreamNdjsonReader::new(reader, &verification_context)" in command
    assert "payload_free_proof_stream_event(&item)" in command
    assert "item.to_json()" not in command
    for secret_payload in (
        '"proof"',
        '"leaf_bytes_hex"',
        '"receipt_b64"',
        '"trace_id"',
        '"nonce_b64"',
        '"authorization"',
        '"credential"',
    ):
        assert secret_payload not in event_projection
    assert '"--stream-token" =>' not in command
    assert '"--max-failures" =>' not in command
    assert "sorafs_cli/stream/v2" not in command
    assert "redacted_endpoint(&endpoint)" in command
    assert "&manifest_bytes," in command
