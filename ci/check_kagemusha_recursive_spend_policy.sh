#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_POLICY_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
MODE="${1:-}"

python3 - "$ROOT_DIR" "$MODE" <<'PY'
import base64
import hashlib
import json
import re
import sys
from fnmatch import fnmatchcase
from pathlib import Path

root = Path(sys.argv[1])
mode = sys.argv[2]
text_overrides = {}

DOC_PATHS = (
    "docs/source/offline_kagemusha.md",
    "roadmap.md",
    "IrohaSwift/README.md",
    "java/iroha_android/README.md",
    "kotlin/README.md",
    "csharp/README.md",
    "javascript/iroha_js/README.md",
    "python/iroha_python/README.md",
)

SHARED_FIXTURE_PATH = "fixtures/kagemusha_recursive_spend_abi6/manifest.json"
SHARED_ARCHIVE_FIXTURE_PATH = "fixtures/kagemusha_recursive_spend_abi6/archives.json"

SHARED_FIXTURE_COVERAGE = {
    SHARED_FIXTURE_PATH: (
        '"schema": "iroha.kagemusha.recursive_spend.abi6.fixture_manifest.v1"',
        '"archive_fixture"',
        "archives.json",
        '"operation_count": 9',
        '"connect_norito_kagemusha_recursive_spend_init"',
        '"connect_norito_kagemusha_recursive_spend_append"',
        '"connect_norito_kagemusha_recursive_spend_transition_profile_init"',
        '"connect_norito_kagemusha_recursive_spend_transition_profile_append"',
        '"connect_norito_kagemusha_recursive_spend_lineage_append_boundary"',
        '"connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result"',
        '"connect_norito_kagemusha_recursive_spend_lineage_witness_append_result"',
        '"connect_norito_kagemusha_recursive_spend_verify"',
        '"connect_norito_kagemusha_recursive_spend_redeem"',
        '"reserved_lineage_payload_bytes": 3847',
        '"reserved_lineage_transition_profile_bytes": 2817',
    ),
    SHARED_ARCHIVE_FIXTURE_PATH: (
        '"schema": "iroha.kagemusha.recursive_spend.abi6.archive_fixtures.v1"',
        '"init_request"',
        '"append_request"',
        '"transition_profile_init"',
        '"transition_profile_append"',
        '"append_bundle"',
        '"lineage_append_boundary"',
        '"lineage_witness_from_init_result"',
        '"lineage_witness_append_result"',
        '"verify_request"',
        '"verify_result"',
        '"redeem_request"',
        '"redeem_instruction"',
        '"request_archive_fields"',
        '"lineage_verifier_key"',
        '"lineage_proving_key_archive"',
        '"previous_recursive_proof_open_envelopes_archive"',
        '"lineage_verifier_record"',
        '"lineage_witness"',
        '"block_height"',
        '"KagemushaRecursiveSpendRedeemRequestV1"',
        '"RedeemKagemushaRecursive"',
        '"sha256_hex": "c5402b3ea6aeb35ce12607344304b858273f8589e2b3887708a86cb19665ce68"',
        '"sha256_hex": "334c343b2c943e7f1de7c0f7a7753c023e3275ba86a877f55dd08dc2f65bb59b"',
        '"sha256_hex": "f5a4a6a25fd9bfd8a121893ddb0c977753c16d8b9dfd835477d2965957c7c03e"',
        '"sha256_hex": "88f293dccb455b6fbcd85d7c06426ce45f02a42fc330e68afda490d504903c03"',
    ),
    "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveSpendProverTests.swift": (
        "testSharedRecursiveSpendAbi6FixtureMatchesSdkSurface",
        "fixtures",
        "kagemusha_recursive_spend_abi6",
        "archives.json",
        "archive_fixtures",
        "redeem_request",
        "redeem_instruction",
        "lineage_append_boundary",
        "operation_count",
        "connect_norito_kagemusha_recursive_spend_redeem",
        "reserved_lineage_payload_bytes",
        "request_archive_fields",
        "lineage_verifier_key",
        "lineage_proving_key_archive",
        "previous_recursive_proof_open_envelopes_archive",
        "lineage_witness",
    ),
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProverTest.java": (
        "sharedRecursiveSpendAbi6FixtureMatchesSdkSurface",
        "kagemusha_recursive_spend_abi6",
        "manifest.json",
        "archives.json",
        "archive_fixtures",
        "redeem_request",
        "redeem_instruction",
        "lineage_append_boundary",
        "operation_count",
        "connect_norito_kagemusha_recursive_spend_redeem",
        "reserved_lineage_payload_bytes",
        "request_archive_fields",
        "lineage_verifier_key",
        "lineage_proving_key_archive",
        "previous_recursive_proof_open_envelopes_archive",
        "lineage_witness",
    ),
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProverTest.kt": (
        "sharedRecursiveSpendAbi6FixtureMatchesSdkSurface",
        "kagemusha_recursive_spend_abi6",
        "manifest.json",
        "archives.json",
        "archive_fixtures",
        "redeem_request",
        "redeem_instruction",
        "lineage_append_boundary",
        "operation_count",
        "connect_norito_kagemusha_recursive_spend_redeem",
        "reserved_lineage_payload_bytes",
        "request_archive_fields",
        "lineage_verifier_key",
        "lineage_proving_key_archive",
        "previous_recursive_proof_open_envelopes_archive",
        "lineage_witness",
    ),
    "javascript/iroha_js/test/kagemushaRecursiveSpend.test.js": (
        "sharedRecursiveSpendManifest",
        "fixtures/kagemusha_recursive_spend_abi6/manifest.json",
        "archives.json",
        "archive_fixtures",
        "redeem_request",
        "redeem_instruction",
        "lineage_append_boundary",
        "manifest.operation_count, 9",
        "connect_norito_kagemusha_recursive_spend_redeem",
        "reserved_lineage_payload_bytes",
        "request_archive_fields",
        "lineage_verifier_key",
        "lineage_proving_key_archive",
        "previous_recursive_proof_open_envelopes_archive",
    ),
    "python/iroha_python/tests/kagemusha_test.py": (
        "test_recursive_kagemusha_shared_abi6_fixture_matches_sdk_surface",
        "kagemusha_recursive_spend_abi6",
        "archives.json",
        "archive_fixtures",
        "redeem_request",
        "redeem_instruction",
        "lineage_append_boundary",
        'manifest["operation_count"] == 9',
        "connect_norito_kagemusha_recursive_spend_redeem",
        "reserved_lineage_payload_bytes",
        "request_archive_fields",
        "lineage_verifier_key",
        "lineage_proving_key_archive",
        "previous_recursive_proof_open_envelopes_archive",
    ),
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/KagemushaRecursiveSpendNativeTests.cs": (
        "RecursiveSpendSharedAbi6FixtureMatchesSdkSurface",
        "kagemusha_recursive_spend_abi6",
        "archives.json",
        "archive_fixtures",
        "redeem_request",
        "redeem_instruction",
        "lineage_append_boundary",
        'root.GetProperty("operation_count").GetInt32()',
        "connect_norito_kagemusha_recursive_spend_redeem",
        "reserved_lineage_payload_bytes",
        "request_archive_fields",
        "lineage_verifier_key",
        "lineage_proving_key_archive",
        "previous_recursive_proof_open_envelopes_archive",
        "lineage_witness",
    ),
}

ADVERSARIAL_COVERAGE = {
    "crates/iroha_data_model/src/offline/mod.rs": (
        "fn kagemusha_recursive_spend_lineage_witness_helpers_append_record_backed_material",
        "KagemushaRecursiveSpendLineageKeyArtifactsV1",
        "is_supported_kagemusha_recursive_spend_lineage_verifier_opening_len",
        "validate_kagemusha_recursive_spend_lineage_key_artifact_pair",
        "KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_VERSION_V1",
        "KagemushaLineageProvingKeyArchiveV1",
        "kagemusha_lineage_vk_envelope_circuit_id",
        "validate_kagemusha_recursive_spend_lineage_key_artifact_package_binding",
        "append_zk1_tlv",
        "kagemusha_lineage_key_artifact_packages_reject_profile_splices",
        "duplicate_cid_vk",
        "wrong_commitment_pk",
        "bad_version_pk",
        "empty_payload_pk",
        '"CID1"',
        '"IPAK"',
        '"H2VK"',
        "new_for_init",
        "new_for_append",
        "new_with_lineage_key_artifacts",
        "new_with_lineage_key_artifact_package",
        "new_with_previous_lineage_proof_witness_and_key_artifacts",
        "new_with_previous_lineage_proof_witness_and_key_artifact_package",
        "with_lineage_key_artifacts",
        "with_lineage_key_artifact_package",
        "with_block_height",
        "ABI init request builder accepts Reserved-lineage key material",
        "ABI init production builder accepts Reserved-lineage key material",
        "ABI init request builder accepts Reserved-lineage key artifact package",
        "ABI init production builder accepts Reserved-lineage key artifact package",
        "ABI append request builder accepts Reserved-lineage key material",
        "ABI append production builder accepts Reserved-lineage key material",
        "ABI append request builder accepts Reserved-lineage key artifact package",
        "ABI append production builder accepts Reserved-lineage key artifact package",
        'field: "proof_circuit_id"',
        'field: "verifier_opening_len"',
        'field: "lineage_key_artifacts"',
        "multi-hop Reserved-lineage redeem is structurally admissible with record-backed lineage witness",
        'field: "lineage_witness"',
        'field: "lineage_verifier_record"',
        'field: "lineage_witness.previous_recursive_proofs.recursive_verifier_scalar_projection_digest"',
        "lineage_witness.previous_recursive_proofs.verifier_opening_len",
        "lineage_witness.previous_recursive_proofs.verifier_params_fingerprint",
        "lineage_witness.previous_recursive_proofs.fixed_window_table_schedule_digest",
        "lineage_witness.previous_recursive_proofs.fixed_window_shared_table_manifest_digest",
        "final_note_nullifier_reuses_initial_input",
        "final_note_nullifier_reuses_prior_output",
        'field: "lineage_witness.current_notes.final"',
        'field: "lineage_witness.record_bundle.verifier_records.conflict"',
                'field: "lineage_witness.record_bundle.verifier_records.duplicate"',
                'field: "lineage_witness.record_bundle.verifier_records.unreferenced"',
                'field: "previous_recursive_proof_open_envelopes_archive"',
                'field: "previous_recursive_proof_open_envelopes_archive.vk_commitment"',
                'field: "previous_recursive_proof_open_envelopes_archive.public_inputs_schema_hash"',
                'field: "previous_recursive_proof_open_envelopes_archive.domain_tag"',
                "reserved_output_append_with_stale_previous_proof_payload",
                "previous_recursive_proof_open_envelopes_archive_digest",
                "append_opening_preflight_digest",
                "transition_profile_binding_digest",
                "recursive_proof_chain_digest",
                "proof bytes must be part of the exported recursive proof artifact digest",
                "proof-byte splice is bound into accumulator state",
                "kagemusha_recursive_spend_bundle_rejects_public_input_tampering",
                "forged proof-chain public-input hash",
                "recursive spend transition-profile binding must be initialized at the first hop",
                "recursive-spend-forged-transition-binding-public-input",
                "kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight",
                "KagemushaRecursiveSpendLineageAppendOpeningPreflightV1",
                "kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight_contract",
                'field: "append_opening_preflight_digest"',
                'field: "append_opening_preflight.current_hop_proof_hash"',
                'field: "append_opening_preflight.previous_recursive_proof_open_envelopes_archive_digest"',
                'field: "append_opening_preflight.previous_recursive_proof_opening_len"',
                'field: "append_opening_preflight.previous_recursive_proof_params_fingerprint"',
                'field: "append_opening_preflight.previous_recursive_proof_fixed_window_table_schedule_digest"',
                'field: "append_opening_preflight.previous_recursive_proof_fixed_window_shared_table_manifest_digest"',
                "append_opening_preflight.shared_opening_len",
                "append_opening_preflight.shared_params_fingerprint",
                "append_opening_preflight.shared_fixed_window_table_schedule_digest",
                "append_opening_preflight.shared_fixed_window_table_manifest_digest",
                'field: "append_opening_preflight.current_hop_verifier_witness_batch_digest"',
                "kagemusha_recursive_previous_proof_open_envelopes_archive_digest",
                "kagemusha_recursive_spend_transition_profile_append_evidence_with_previous_proof_openings",
                "legacy evidence-only append profiles omit host opening-archive bytes",
                "legacy evidence-only append profiles omit append opening preflight bytes",
                "binding append opening preflight bytes must change the transition profile digest",
                "binding the full append opening preflight contract must change the transition profile digest",
                "KagemushaRecursiveSpendLineageAppendBoundaryV1",
                "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1",
                "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1",
                "kagemusha_recursive_spend_lineage_append_boundary_chain_asset_binding_digest",
                "kagemusha_recursive_spend_lineage_append_boundary_final_note_binding_digest",
                "kagemusha_recursive_spend_lineage_append_boundary_from_transition_profile",
                "validate_against_transition_profile",
                "kagemusha_recursive_spend_transition_profile_binds_adversarial_mutations",
                "validate_kagemusha_unique_input_output_sets",
                "duplicate_initial_input_profile",
                "overlapping_initial_output_profile",
                "duplicate_append_output_profile",
                "assert_self_consistent_forged_boundary_rejected",
                "forged_boundary_cases",
                "kagemusha_recursive_spend_accumulator_append_evidence_with_opening_preflight_contract",
                "kagemusha_recursive_spend_append_boundary_free_public_inputs_hash",
                "append_boundary.transition_profile_digest",
                "append_boundary.transition_profile_binding_digest",
                "append_boundary.chain_asset_binding_digest",
                "append_boundary.final_note_binding_digest",
                "append_boundary.previous_recursive_proof_artifact_digest",
                "append_boundary.previous_recursive_proof_open_envelopes_archive_digest",
                'field: "append_boundary.append_boundary_digest"',
                'field: "append_boundary.previous_accumulator_digest"',
                'field: "append_boundary.append_opening_preflight_digest"',
                "append_boundary.current_hop_proof_hash",
                'field: "append_boundary.current_hop_opening_aggregate_digest"',
                "append_boundary.resulting_accumulator_digest",
                'field: "append_boundary.resulting_public_inputs_hash"',
                "append_boundary.verifier_opening_len",
                'field: "append_boundary.verifier_params_fingerprint"',
                "append_boundary.fixed_window_table_schedule_digest",
                "append_boundary.fixed_window_shared_table_manifest_digest",
                "self-consistent-forged-append-boundary-shared-table",
                'field: "append_boundary.hop_count"',
                "self_consistent_forged_current_opening",
                "append-boundary digest must not feed back into the accumulator digest",
                "lineage append proof binds canonical append-boundary public input",
                "stale_append_boundary.current_hop_opening_aggregate_digest",
                'field: "append_boundary.previous_recursive_proof_opening_aggregate_digest"',
                'field: "append_boundary.chain_asset_binding_digest"',
                'field: "append_boundary.final_note_binding_digest"',
                "opening transcript bytes remain bound even when metadata is identical",
                "not a norito previous-proof opening archive",
                "empty previous-proof opening transcript label",
                "non-Pallas previous-proof opening curve id",
                "previous-proof opening generator count mismatch",
                "previous-proof opening IPA round count mismatch",
                'field: "previous_bundle.recursive_proof.proof.circuit_id"',
                "forged-previous-recursive-proof-circuit-id",
                'field: "previous_accumulator_public_inputs_hash"',
                "Reserved-lineage append request at the witnessless hop cap must reject before proving",
        "fn kagemusha_recursive_spend_rejects_malformed_notes_and_lineage",
        'field: "lineage_witness.record_bundle.verifier_records.missing"',
        "empty lineage Pallas opening transcript label",
        "zero lineage Pallas opening verifier-key metadata",
        "lineage Pallas opening parameter/public length mismatch",
        "lineage Pallas opening generator count mismatch",
        "lineage Pallas opening IPA round count mismatch",
        "lineage Pallas opening verifier-key metadata substitution",
        "lineage Pallas opening public-input schema metadata substitution",
        'field: "lineage_witness.pallas_open_envelopes_archive.vk_commitment"',
        'field: "lineage_witness.pallas_open_envelopes_archive.public_inputs_schema_hash"',
        'field: "lineage_witness.record_bundle.verifier_records.key_commitment"',
        'field: "lineage_witness.record_bundle.bundle.steps.attachment.proof.circuit_id"',
        "forged-hop-proof-vk-hash",
        "forged-hop-proof-circuit-id",
        "stale_hop_proof_schema",
        "previous_proof_count_mismatch",
    ),
    "crates/iroha_core/src/zk.rs": (
        "fn kagemusha_recursive_spend_chain_admission_validates_enabled_lineage_profile",
        "fn kagemusha_recursive_spend_instance_values_expose_proof_chain_digest",
        "recursive spend proof-chain public input binds accumulator",
        "proof_chain_public_input_tamper",
        "kagemusha_non_native_vesta_ipa_verifier_from_pallas_witness_rejects_generator_fold_splice",
        "accumulator G fold mismatch",
        "kagemusha_non_native_vesta_ipa_verifier_batch_preflight_rejects_h_generator_fold_splice",
        "kagemusha_non_native_vesta_ipa_verifier_shared_table_from_pallas_witness_rejects_h_generator_fold_splice",
        "kagemusha_non_native_vesta_ipa_verifier_shared_table_batch_preflight_rejects_h_generator_fold_splice",
        "kagemusha_non_native_vesta_ipa_verifier_from_pallas_witness_rejects_h_generator_fold_splice",
        "accumulator H fold mismatch",
        "decode_kagemusha_recursive_compact_pallas_open_envelopes",
        "failed to decode Kagemusha recursive compact Pallas open-envelope archive",
        "invalid Kagemusha recursive compact Pallas open-envelope archive",
        "invalid Kagemusha recursive compact record-backed Pallas preflight",
        "KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_PROOF_UNAVAILABLE",
        "multi-hop proving requires the append verifier batch to be composed into the compact proof",
        "fn kagemusha_recursive_compact_record_prover_preflights_pallas_archive_before_unavailable",
        "fn kagemusha_recursive_compact_record_bound_pallas_preflights_before_unavailable",
        "heavy Kagemusha Halo2 IPA proof generation; run explicitly with --ignored --test-threads=1",
        "Norito-valid data-model proof envelope must not decode as Pallas openings",
        "detached compact Pallas archive must reject before unavailable",
        "extra compact Pallas opening must reject before unavailable",
        "missing compact Pallas opening must reject before unavailable",
        "duplicated multi-hop compact Pallas archive must reject before unavailable",
        "height-aware duplicated multi-hop compact Pallas archive must reject before unavailable",
        "forged multi-hop compact Pallas metadata must reject before unavailable",
        "reordered multi-hop compact Pallas archive must reject before unavailable",
        "height-aware reordered multi-hop compact Pallas archive must reject before unavailable",
        "record-bound multi-hop compact Pallas archive must reject before the unavailable gate",
        "missing compact one-hop proving key archive",
        "ensure_kagemusha_recursive_compact_token_public_instance_context",
        "verifier_witness_batch_digest public instance digest must be non-zero",
        "forged verifier-witness batch digest must reject",
        "unsupported verifier opening length must reject",
        "non-u64 verifier metadata limb must reject",
        "recursive public-input hash must reject",
        "recursive_public_inputs_hash",
        "recursive compact token multi-row public instances must reject",
        "CID-spoofed ABI-7 compact verifier key must reject",
        "ABI-7 compact token without scalar projection must reject",
        "recursive_verifier_scalar_projection_digest public instance digest must be non-zero for ABI-7 compact circuit",
        "canonical u64 limb",
        "preverify_kagemusha_recursive_spend_compact_payment_token_projection",
        "verify_kagemusha_recursive_spend_compact_payment_token_projection",
        "fn kagemusha_recursive_spend_compact_projection_token_preverify_accepts_lineage_profiles",
        "ABI-7 compact verifier must remain closed to lineage projected tokens",
        "folded projection splice must reject even after token hash recomputation",
        "one-hop side scalar projection splice must reject",
        "append lineage projected token preverification",
        "height-unbound lineage projection verifier must reject windowed records",
        "stale previous recursive proof public-input hash must not pass append preflight",
        "context-spliced previous proof still has a valid generic profile",
        "previous proof verifier-context splice must reject",
        "validate_append_pallas_witness_preflight_bindings",
        "append recursive verifier slice binds detached previous and hop-bound current preflights",
        "append slice must reject mismatched previous recursive proof witness",
        "append slice must reject stale current-hop proof hash",
        "append slice must reject detached current-hop preflight",
        "record-backed LEN=4 evidence binds to one-hop verifier-slice metadata",
        "one-hop verifier-slice evidence binding must reject batch digest splice",
        "one-hop verifier-slice evidence binding must reject proof-count splice",
        "one-hop verifier-slice evidence binding must reject witness-profile splice",
        "one-hop verifier-slice evidence binding must reject params fingerprint splice",
        "one-hop verifier-slice evidence binding must reject schedule digest splice",
        "one-hop verifier-slice evidence binding must reject shared-table manifest splice",
        "one-hop verifier-slice evidence binding must reject opening-length splice",
        "one-hop verifier-slice evidence binding must reject table-base splice",
        "one-hop verifier-slice open-envelope evidence must reject params splice",
        "requires one preflight witness",
        "verifier-witness profile mismatch",
        "verifier parameter fingerprint mismatch",
        "fixed-window schedule digest mismatch",
        "shared-table manifest digest mismatch",
        "fixed-window table-base digest mismatch",
        "fn kagemusha_verified_folded_public_inputs_rejects_input_output_overlap_before_proof_decode",
        "output commitment overlaps an input nullifier",
        "input nullifier overlaps an output commitment",
        "record-backed cross-hop overlap error should come before proof decoding",
        "fn kagemusha_recursive_spend_lineage_witness_preflights_fold_metadata_before_archive_decode",
        "validate_kagemusha_fold_metadata(&fold_steps)",
        "lineage witness root-continuity error should come before Pallas archive decoding",
        "lineage witness root-continuity error should come before previous-proof checks",
        "validate_kagemusha_hop_verifier_record_set(&fold_steps, &hop_verifier_records)",
        "fn kagemusha_recursive_spend_lineage_witness_preflights_verifier_records_before_archive_decode",
        "lineage witness verifier-record error should come before Pallas archive decoding",
        "lineage witness verifier-record error should come before previous-proof checks",
        "current note {hop_index} note commitment must be non-zero",
        "current note {hop_index} amount must be a non-zero integer u128",
        "fn kagemusha_recursive_spend_lineage_witness_preflights_current_notes_before_archive_decode",
        "lineage witness current-note error should come before Pallas archive decoding",
        "lineage witness current-note error should come before previous-proof checks",
        "current note {hop_index} commitment is not created by its lineage hop",
        "current note {hop_index} spend nullifier collides with its lineage input nullifiers",
        "current note {hop_index} amount does not match the previous current note",
        "fn kagemusha_recursive_spend_lineage_witness_preflights_current_note_bindings_before_archive_decode",
        "lineage witness current-note binding error should come before Pallas archive decoding",
        "lineage witness current-note binding error should come before previous-proof checks",
        "hop {hop_index} append must have exactly one input nullifier",
        "hop {hop_index} must consume the previous current-note spend nullifier",
        "current note {hop_index} spend nullifier collides with the previous note commitment",
        "hop {hop_index} output commitments recreate the previous current note",
        "fn kagemusha_recursive_spend_lineage_witness_preflights_append_handoff_before_archive_decode",
        "lineage witness append-handoff error should come before Pallas archive decoding",
        "lineage witness append-handoff error should come before previous-proof checks",
        "ensure_record_backed_recursive_spend_lineage_witness_matches_final_bundle",
        "lineage witness final current note does not match redeem bundle",
        "fn kagemusha_recursive_spend_lineage_witness_preflights_final_bundle_before_archive_decode",
        "lineage witness final-bundle error should come before Pallas archive decoding",
        "lineage witness final-bundle error should come before previous-proof checks",
        "verifier opening length",
        "verifier parameter fingerprint",
        "fixed-window table schedule digest",
        "fixed-window shared-table manifest digest",
        "final note spend nullifier collides with a lineage input nullifier",
        "spend nullifier collides with a lineage output commitment",
        "lineage profile without scalar projection must reject",
        "metadata-valid one-hop lineage profile must be chain-admission capable",
        "two-hop lineage profile must use the append verifier-slice layout",
        "fn kagemusha_recursive_spend_lineage_backend_profile_rejects_multi_hop_metadata_splices",
        "missing previous-proof verifier-key metadata must reject",
        "zero previous-proof public-input schema metadata must reject",
        "non-Pallas previous-proof opening must reject",
        "over-count previous-proof archive must reject",
        "kagemusha_recursive_spend_transition_profile_append_evidence_with_previous_proof_openings",
        "the one-hop Reserved-lineage verifier dispatch must reject forged multi-hop metadata",
        "lineage scalar projection public column must be non-zero",
        "KagemushaRecursiveSpendLineageAppendOpeningPreflight",
        "preflight_kagemusha_recursive_spend_append_transition_profile_with_opening_preflight",
        "append_opening_preflight.contract",
        "transition binding public instance tamper must reject",
        "transition binding public input tampering must reject",
        "kagemusha_recursive_spend_lineage_append_opening_preflight_contract",
        "KagemushaRecursiveSpendLineageAppendOpeningPreflightV1::new",
        "fn kagemusha_recursive_spend_lineage_append_opening_preflight_binds_archives",
        "current-hop preflight digest must bind the checked-hop proof hash",
        "append opening preflight must reject previous-proof metadata splices",
        "preflight_kagemusha_recursive_spend_lineage_append_accumulator_opening_contract",
        "ensure_kagemusha_recursive_spend_lineage_append_boundary_matches_accumulator",
        "append_boundary.resulting_accumulator_digest",
        "append_boundary.append_boundary_digest != accumulator.append_boundary_digest",
        "append_boundary.transition_profile_binding_digest",
        "append_boundary.chain_asset_binding_digest",
        "append_boundary.final_note_binding_digest",
        "append_boundary.resulting_public_inputs_hash",
        "kagemusha_recursive_spend_append_boundary_free_public_inputs_hash",
        "requires a Reserved-lineage previous proof",
        "append public inputs append boundary digest mismatch",
        "Kagemusha Reserved-lineage append public inputs {label} mismatch",
        "KagemushaRecursiveAggregationAppendVerifierSlice",
        "KagemushaRecursiveAggregationAppendVerifierSliceConfig",
        "KagemushaRecursiveSpendLineageBackendProfile",
        "kagemusha_recursive_spend_lineage_append_vk_box",
        "build_kagemusha_recursive_spend_lineage_append_vk_box",
        "kagemusha_recursive_spend_lineage_append_backend_opening_len",
        "cached_kagemusha_recursive_spend_lineage_append_proving_key",
        "prove_halo2_ipa_kagemusha_recursive_spend_lineage_append_envelope",
        "prove_halo2_ipa_kagemusha_recursive_spend_lineage_append_envelope_dispatch",
        "kagemusha_append_verifier_scalar_projection_digest",
        "kagemusha_append_verifier_scalar_projection_inputs",
        "kagemusha_recursive_aggregation_append_verifier_slice_link",
        "verify_append_lineage_len",
        "fn kagemusha_recursive_aggregation_append_verifier_slice_builder_accepts_two_opening_profile",
        "fn kagemusha_recursive_aggregation_append_verifier_slice_builder_rejects_adversarial_profile_splices",
        "fn kagemusha_recursive_aggregation_append_verifier_slice_circuit_accepts_two_opening_profile",
        "fn kagemusha_recursive_aggregation_append_verifier_slice_circuit_rejects_scalar_projection_public_input_splice",
        "fn kagemusha_recursive_aggregation_append_verifier_slice_circuit_rejects_current_verifier_transcript_digest_splice",
        "previous recursive proof preflight requires exactly one witness",
        "append-boundary digest must be non-zero",
        "scalar-projection digest mismatch",
        "fn kagemusha_recursive_spend_lineage_append_accumulator_opening_contract_rejects_splices",
        "refresh_append_opening_contract_digest",
        "forged append transition boundary digest must reject",
        "forged append boundary chain binding must reject",
        "forged append boundary current-note binding must reject",
        "forged append boundary final-root binding must reject",
        "forged previous accumulator digest must reject",
        "forged contract previous accumulator digest must reject",
        "previous accumulator digest mismatch",
        "forged previous proof artifact digest must reject",
        "forged contract previous proof artifact digest must reject",
        "previous proof artifact digest mismatch",
        "forged previous proof opening archive digest must reject",
        "forged contract previous proof opening archive digest must reject",
        "previous proof opening archive digest mismatch",
        "forged previous proof verifier preflight must reject",
        "forged contract previous proof verifier preflight must reject",
        "previous proof preflight mismatch",
        "forged contract current-hop verifier preflight must reject",
        "metadata-spliced current-hop opening archive must reject",
        "current-hop opening metadata splice error",
        "forged current-hop proof hash must reject",
        "forged contract current-hop proof hash must reject",
        "current-hop proof hash mismatch",
        "forged accumulator append-opening digest must reject",
        "forged accumulator verifier parameter fingerprint must reject",
        "forged accumulator opening length must reject",
        "opening length mismatch",
        "forged accumulator fixed-window schedule digest must reject",
        "forged accumulator shared-table manifest digest must reject",
        "forged current-hop preflight schedule must reject",
        "stale previous accumulator hop count must reject",
        "ensure_kagemusha_recursive_spend_lineage_witnessless_append_available",
        "fn kagemusha_recursive_spend_lineage_append_availability_enforces_hop_cap",
        "one-hop verifier slice",
        "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_RUNTIME_KEYGEN_ENV",
        "fn kagemusha_recursive_spend_lineage_init_default_rejects_missing_key_artifacts_before_runtime_keygen",
        "packaged verifier/proving key artifacts",
        "at least 2 hops",
        "output_append_is_currently_provable",
        "two-hop Reserved-lineage append is enabled",
        "64-hop Reserved-lineage append is inside the policy cap",
        "exceeds witnessless Reserved-lineage hop cap",
        "fn kagemusha_recursive_spend_append_rejects_lineage_output_at_cap_before_inputs",
        "direct Reserved-lineage append at the witnessless hop cap must reject before input parsing",
        "capped Reserved-lineage append must reject before verifier-record or archive checks",
        "fn kagemusha_recursive_spend_verify_result_requires_matching_lineage_record",
    ),
    "crates/iroha_core/src/smartcontracts/isi/offline.rs": (
        "fn kagemusha_recursive_redeem_rejects_semantic_recursive_spend_before_mint",
        'assert_offline_rejection(err, "invalid_recursive_bundle", "private-hop lineage")',
        "rejected recursive redeem must not consume the final spendable note nullifier",
        "assert_eq!(balance, Numeric::zero())",
        "fn kagemusha_recursive_redeem_record_backed_multi_hop_mints_and_rejects_replay",
        "real_recursive_kagemusha_redeem_record_backed_multi_hop_fixture",
        "recursive_redeem_real_lineage_transfer_step",
        "record-backed multi-hop recursive Kagemusha redeem should mint",
        "redeem must consume every top-up anchor nullifier",
        "record-backed multi-hop recursive Kagemusha redeem must not replay",
        'assert_offline_rejection(err, "duplicate_nullifier", "already spent")',
        "assert_eq!(balance, Numeric::new(42, 0))",
        "fn kagemusha_recursive_redeem_reserved_lineage_profile_verifies_backend_before_mint",
        "one-hop verifier-slice",
        "fn kagemusha_recursive_redeem_rejects_adversarial_lineage_verifier_records_before_mint",
        "not registered",
        "does not match the registered record",
        "is missing",
        "is duplicated",
        "not referenced",
        "fn kagemusha_recursive_redeem_rejects_malformed_lineage_hop_proof_before_mint",
        "not-a-valid-kagemusha-lineage-hop-proof",
        "OpenVerifyEnvelope",
        "fn kagemusha_recursive_redeem_rejects_lineage_final_nullifier_collisions_before_mint",
        "final note spend nullifier collides with a lineage input nullifier",
        "spend nullifier collides with a lineage output commitment",
        "fn kagemusha_recursive_redeem_rejects_verifier_and_policy_misconfigurations",
        "recursive Kagemusha redeem must reject missing recursive verifier",
        "recursive Kagemusha redeem must reject inactive recursive verifier",
        "recursive Kagemusha redeem must reject substituted unshield verifier key",
        "fn kagemusha_recursive_redeem_rejects_amount_and_final_binding_mismatches",
        "wrong recursive Kagemusha public amount must reject",
        "mutated recursive Kagemusha redeem public inputs must reject",
        "tampered recursive Kagemusha redeem proof must reject",
    ),
    "crates/connect_norito_bridge/src/lib.rs": (
        "kagemusha_recursive_compact_ffi_fails_closed_and_rejects_adversarial_inputs",
        "malformed Pallas opening archives before the unavailable gate",
        "detached valid Pallas opening archives before the unavailable gate",
        "prove_verified_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelope_archive",
        "is_kagemusha_recursive_compact_unavailable_error",
        "Vec<iroha_zkp_halo2::OpenVerifyEnvelope>",
        "valid recursive compact Pallas envelope fixture must decode",
        "ABI-7 compact prover must reject extra valid Pallas opening archives before the unavailable gate",
        "ABI-7 compact prover must reject missing valid Pallas opening archives before the unavailable gate",
        "ABI-7 compact prover must reject duplicated multi-hop valid Pallas opening archives before the unavailable gate",
        "ABI-7 compact prover must reject forged multi-hop Pallas metadata before the unavailable gate",
        "ABI-7 compact prover must reject reordered valid Pallas opening archives before the unavailable gate",
        "valid multi-hop recursive compact Pallas archives must map to unavailable",
        "shape-valid ABI-7 compact tokens with invalid proof bodies must return a soft invalid result",
        "ABI-7 compact verifier must reject non-canonical envelope verifier-key hashes before returning a soft invalid result",
        "non-canonical compact-token verifier-key hashes must clear stale valid flags",
        "ABI-7 compact verifier must reject multi-row public instances before returning a soft invalid result",
        "multi-row compact-token public instances must clear stale valid flags",
        "shape-valid envelopes with stale folded-token bindings must hard-fail before soft invalid",
        "malformed public-input bindings before returning a soft invalid result",
        "preverify_kagemusha_recursive_compact_payment_token",
        "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE",
        "ensure_kagemusha_recursive_spend_pallas_archive",
        "fn kagemusha_recursive_spend_ffi_rejects_empty_nested_pallas_archives_without_output",
        "assert_recursive_spend_single_archive_ffi_rejects_empty_nested_pallas",
        "empty nested Pallas archive must not return output bytes",
        "pallas_open_envelopes_archive.clear()",
        "bridge-recursive-spend-append-current-hop-open",
        "fn kagemusha_recursive_spend_redeem_bridge_rejects_adversarial_lineage_witnesses",
        "missing verifier record",
        "duplicate verifier record",
        "unreferenced verifier record",
        "malformed Pallas envelope archive",
        "current note commitment mismatch",
        "final note input-nullifier collision",
        "final note output-commitment collision",
        "stale previous recursive proof public-input hash must not return a transition profile",
        "kagemusha_recursive_spend_transition_profile_append_evidence_with_previous_proof_openings",
        "kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight_contract",
        "kagemusha_recursive_spend_lineage_append_opening_preflight_from_archives",
        "connect_norito_kagemusha_recursive_spend_lineage_append_boundary",
        "kagemusha_recursive_spend_lineage_append_boundary_from_transition_profile_archive",
        "validate_against_transition_profile",
        "profiles with duplicate current-hop outputs",
        "duplicate_output_profile",
        "kagemusha_fold_step_proof_hash",
        "can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(",
        "output_append_is_currently_provable",
        "VerifyingKeyBox::new(",
        "fn kagemusha_recursive_spend_transition_profile_append_ffi_binds_append_opening_preflight",
        "bridge append profile binds append opening preflight digest",
        "bridge append profile binds full append opening preflight contract",
        "bridge append profile must reject forged current-hop opening metadata",
        "legacy append profiles must not synthesize append opening preflight bytes",
        "legacy append profiles must not synthesize append opening preflight contracts",
        "fn kagemusha_recursive_spend_append_ffi_rejects_forged_previous_proof_opening_metadata",
        '"vk_commitment"',
        '"public_inputs_schema_hash"',
        '"domain_tag"',
        "forged previous-proof opening {case} must not return output bytes",
        "fn kagemusha_recursive_spend_append_ffi_rejects_malformed_previous_proof_opening_archives",
        "malformed previous-proof opening archive",
        "empty previous-proof opening vector",
        "over-count previous-proof opening vector",
        "{case} must not return output bytes",
        "fn kagemusha_recursive_spend_append_ffi_rejects_stale_previous_proof_payload_opening",
        "stale previous-proof payload opening must not return output bytes",
        "fn kagemusha_recursive_spend_append_ffi_rejects_forged_previous_proof_circuit_id",
        "forged previous recursive proof circuit id must not return output bytes",
        "fn kagemusha_recursive_spend_append_ffi_rejects_semantic_previous_to_lineage_output",
        "semantic previous proofs must not upgrade into Reserved-lineage output",
        "fn kagemusha_recursive_spend_append_ffi_rejects_lineage_output_at_hop_cap",
        "Reserved-lineage append request at the witnessless hop cap must not return output bytes",
        "fn kagemusha_recursive_spend_append_ffi_rejects_missing_lineage_key_artifacts",
        "missing verifier key",
        "must not return Reserved-lineage append output bytes",
        "fn kagemusha_recursive_spend_init_ffi_rejects_forged_current_hop_pallas_metadata",
        "current-hop Pallas metadata splice must not return bytes",
        "fn kagemusha_recursive_spend_init_ffi_rejects_forged_current_hop_proof_circuit_id",
        "current-hop proof circuit-id splice must not return bytes",
        "fn kagemusha_recursive_spend_init_ffi_rejects_missing_lineage_key_artifacts",
        "missing Reserved-lineage key artifacts must not return bytes",
        "bridge-fast-lineage-hop-public-inputs-v1",
        "encode first fast lineage hop proof",
        "encode second fast lineage hop proof",
        "unexpected previous recursive proof for one-hop witness",
        "reserved lineage bundle with record-backed witness",
        "fn kagemusha_recursive_spend_redeem_bridge_accepts_witnessless_reserved_lineage_public_binding",
        "witnessless reserved-lineage redeem validates before backend proof verification",
        "bridge must reject the fixture's backend-invalid reserved-lineage proof",
    ),
    "crates/iroha_js_host/src/lib.rs": (
        "kagemusha_recursive_compact_payment_token_js_host_rejects_malformed_inputs",
        "const KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES: usize = 64 * 1024 * 1024;",
        "ensure_kagemusha_recursive_archive_len",
        "archive_len > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
        "encoded Kagemusha archive exceeds",
        "KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES + 1",
        "oversized record bundle must reject before Norito decode",
        "Kagemusha record bundle archive must not exceed",
        "oversized recursive compact Pallas archive must reject before core preflight",
        "pallasOpenEnvelopesArchive must not exceed",
        "oversized recursive compact token must reject before Norito decode",
        "Kagemusha recursive compact payment token archive must not exceed",
        "invalid Kagemusha recursive compact Pallas open-envelope archive",
        "invalid Kagemusha recursive compact record-backed Pallas preflight",
        "detached valid recursive compact Pallas archive must reject",
        "valid multi-hop recursive compact archive must remain unavailable",
        "KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_PROOF_UNAVAILABLE",
        "recursive compact prover must reject extra valid Pallas opening archive",
        "recursive compact prover must reject missing valid Pallas opening archive",
        "recursive compact prover must reject duplicated multi-hop valid Pallas opening archive",
        "recursive compact prover must reject forged multi-hop Pallas metadata",
        "recursive compact prover must reject reordered valid Pallas opening archive",
        "Vec<iroha_zkp_halo2::OpenVerifyEnvelope>",
        "recursive compact token with malformed binding must reject",
        "recursive compact token with forged verifier-key hash must reject",
        "envelope verifier-key hash mismatch",
        "JS host recursive compact verifier must reject multi-row public instances",
        "unexpected JS host multi-row compact-token error",
        "preverify_kagemusha_recursive_compact_payment_token",
        "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE",
        "fn kagemusha_recursive_spend_js_host_rejects_oversized_archives_before_decode",
        "assert_oversized_archive_rejected_for_js_host",
        "must reject oversized archives before Norito decode",
        "Kagemusha recursive spend init archive",
        "Kagemusha recursive spend transition profile append archive",
        "Kagemusha recursive spend lineage append boundary archive",
        "Kagemusha recursive spend lineage witness init bundle archive",
        "Kagemusha recursive spend previous lineage witness archive",
        "Kagemusha recursive spend lineage witness append bundle archive",
        "ensure_kagemusha_recursive_spend_pallas_archive",
        "fn kagemusha_recursive_spend_js_host_rejects_empty_nested_pallas_archives_before_core",
        "assert_empty_nested_pallas_archive_rejected_for_js_host",
        "must reject empty nested Pallas archives before core",
        "Kagemusha recursive spend Pallas open-envelope archive must not be empty",
        "pallas_open_envelopes_archive.clear()",
        "fn kagemusha_recursive_spend_verify_requires_lineage_record_for_reserved_lineage",
        "forged lineage verifier record was not rejected clearly",
        "fn kagemusha_recursive_spend_redeem_instruction_rejects_semantic_profile_after_public_binding",
        "wrong recursive spend redeem amount must reject",
        "missing recursive spend top-up anchor must reject",
        "zero recursive spend redeem VK commitment must reject",
        "fn kagemusha_recursive_spend_redeem_instruction_requires_lineage_record_for_reserved_previous_proof",
        "JS host must reject forged lineage verifier-record commitment",
        "kagemusha_recursive_spend_transition_profile_append_evidence_with_previous_proof_openings",
        "kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight_contract",
        "kagemusha_recursive_spend_lineage_append_opening_preflight_from_archives",
        "kagemushaRecursiveSpendLineageAppendBoundary",
        "validate_against_transition_profile",
        "kagemusha_fold_step_proof_hash",
        "can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(",
        "output_append_is_currently_provable",
        "Kagemusha recursive spend append cannot prove output proof circuit",
        "VerifyingKeyBox::new(",
        "fn kagemusha_recursive_spend_transition_profile_append_binds_append_opening_preflight",
        "JS host append profile binds append opening preflight digest",
        "JS host append profile binds full append opening preflight contract",
        "JS host append profile must reject forged current-hop opening metadata",
        "JS host legacy append profiles must not synthesize append opening preflight bytes",
        "JS host legacy append profiles must not synthesize append opening preflight contracts",
        "fn kagemusha_recursive_spend_lineage_append_boundary_rejects_duplicate_current_outputs",
        ".push(profile.current_hop_statement.output_commitments[0])",
        "JS host append-boundary helper must reject duplicate current-hop outputs",
        "fn kagemusha_recursive_spend_append_rejects_forged_previous_proof_opening_metadata",
        '"vk_commitment"',
        '"public_inputs_schema_hash"',
        '"domain_tag"',
        "fn kagemusha_recursive_spend_append_rejects_malformed_previous_proof_opening_archives",
        "JS host must reject {case}",
        "malformed previous-proof opening archive",
        "empty previous-proof opening vector",
        "over-count previous-proof opening vector",
        "fn kagemusha_recursive_spend_append_rejects_stale_previous_proof_payload_opening",
        "JS host must reject stale previous-proof payload opening",
        "fn kagemusha_recursive_spend_append_rejects_forged_previous_proof_circuit_id",
        "JS host must reject forged previous recursive proof circuit id",
        "forged previous proof circuit-id returned unexpected error",
        "fn kagemusha_recursive_spend_append_rejects_missing_lineage_key_artifacts",
        "JS host must reject missing Reserved-lineage key artifacts",
        "fn kagemusha_recursive_spend_init_rejects_forged_current_hop_pallas_metadata",
        "JS host must reject forged current-hop Pallas metadata",
        "current-hop metadata splice returned unexpected error",
        "fn kagemusha_recursive_spend_init_rejects_forged_current_hop_proof_circuit_id",
        "JS host must reject forged current-hop proof circuit id",
        "current-hop proof circuit-id splice returned unexpected error",
        "fn kagemusha_recursive_spend_init_rejects_missing_lineage_key_artifacts",
        "JS host must reject missing Reserved-lineage key artifacts",
        "js-host-recursive-lineage-hop-public-inputs-v1",
        "encode JS host lineage hop proof",
        "js-host-mixed-lineage-hop-public-inputs-v1",
        "encode first JS host mixed lineage hop proof",
        "encode second JS host mixed lineage hop proof",
        "encode JS host mixed lineage Pallas archive",
        "fn kagemusha_recursive_spend_redeem_instruction_rejects_backend_invalid_lineage",
        "witnessless reserved-lineage redeem validates before backend proof verification",
        "reserved-lineage Kagemusha recursive spend proof did not verify",
        "reserved-lineage redeem without verifier-slice columns must reject",
        "fn kagemusha_recursive_spend_redeem_instruction_rejects_malformed_lineage_witnesses",
        "final note input-nullifier collision",
        "final note output-commitment collision",
    ),
    "python/iroha_python/iroha_python_rs/src/lib.rs": (
        "kagemusha_recursive_compact_python_function_rejects_malformed_record_bundle",
        "const KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES: usize = 64 * 1024 * 1024;",
        "ensure_kagemusha_recursive_archive_len",
        "archive_len > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
        "encoded Kagemusha archive exceeds",
        "KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES + 1",
        "oversized recursive compact record bundle must reject before Norito decode",
        "Kagemusha recursive compact record bundle archive must not exceed",
        "oversized recursive compact Pallas archive must reject before core preflight",
        "pallas_open_envelopes_archive must not exceed",
        "oversized recursive compact token must reject before Norito decode",
        "Kagemusha recursive compact payment token archive must not exceed",
        "invalid Kagemusha recursive compact Pallas open-envelope archive",
        "invalid Kagemusha recursive compact record-backed Pallas preflight",
        "prove_verified_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelope_archive",
        "failed to decode Kagemusha recursive compact Pallas open-envelope archive",
        "detached valid Pallas archive",
        "valid multi-hop recursive compact archive must remain unavailable",
        "KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_PROOF_UNAVAILABLE",
        "recursive compact prover must reject extra valid Pallas opening archive",
        "recursive compact prover must reject missing valid Pallas opening archive",
        "recursive compact prover must reject duplicated multi-hop valid Pallas opening archive",
        "recursive compact prover must reject forged multi-hop Pallas metadata",
        "recursive compact prover must reject reordered valid Pallas opening archive",
        "recursive compact verifier must reject malformed token binding",
        "recursive compact token with forged verifier-key hash must reject",
        "envelope verifier-key hash mismatch",
        "Python recursive compact verifier must reject multi-row public instances",
        "unexpected Python recursive compact multi-row error",
        "preverify_kagemusha_recursive_compact_payment_token",
        "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE",
        "fn kagemusha_recursive_spend_python_functions_reject_oversized_archives_before_decode",
        "assert_oversized_archive_rejected_python",
        "must reject oversized archives before decode",
        "Kagemusha recursive spend init archive",
        "Kagemusha recursive spend transition profile append archive",
        "Kagemusha recursive spend lineage append boundary archive",
        "Kagemusha recursive spend lineage witness init bundle archive",
        "Kagemusha recursive spend previous lineage witness archive",
        "Kagemusha recursive spend lineage witness append bundle archive",
        "ensure_kagemusha_recursive_spend_pallas_archive",
        "fn kagemusha_recursive_spend_python_functions_reject_empty_nested_pallas_archives_before_core",
        "assert_empty_nested_pallas_archive_rejected_python",
        "must reject empty nested Pallas archives",
        "Kagemusha recursive spend Pallas open-envelope archive must not be empty",
        "pallas_open_envelopes_archive.clear()",
        "fn kagemusha_recursive_spend_verify_python_function_requires_lineage_record",
        "forged lineage verifier record was not rejected clearly",
        "fn kagemusha_recursive_spend_redeem_python_native_rejects_semantic_profile",
        "Python native redeem builder must reject wrong public amount",
        "Python native redeem builder must reject missing top-up anchors",
        "Python native redeem builder must reject zero redeem VK commitment",
        "fn kagemusha_recursive_spend_redeem_python_requires_lineage_record_for_reserved_previous_proof",
        "Python native redeem builder must reject forged lineage verifier record",
        "can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(",
        "output_append_is_currently_provable",
        "Kagemusha recursive spend append cannot prove output proof circuit",
        "VerifyingKeyBox::new(",
        "python-mixed-lineage-hop-public-inputs-v1",
        "encode first Python mixed lineage hop proof",
        "encode second Python mixed lineage hop proof",
        "encode Python mixed lineage Pallas archive",
        "fn kagemusha_recursive_spend_redeem_python_rejects_adversarial_lineage_witnesses",
        "missing verifier record",
        "duplicate verifier record",
        "unreferenced verifier record",
        "final note input-nullifier collision",
        "final note output-commitment collision",
        "malformed Pallas archive",
        "kagemusha_recursive_spend_transition_profile_append_evidence_with_previous_proof_openings",
        "kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight_contract",
        "kagemusha_recursive_spend_lineage_append_opening_preflight_from_archives",
        "kagemusha_recursive_spend_lineage_append_boundary_py",
        "validate_against_transition_profile",
        "kagemusha_fold_step_proof_hash",
        "fn kagemusha_recursive_spend_transition_profile_append_python_binds_append_opening_preflight",
        "Python append profile binds append opening preflight digest",
        "Python append profile binds full append opening preflight contract",
        "Python append profile must reject forged current-hop opening metadata",
        "Python legacy append profiles must not synthesize append opening preflight bytes",
        "Python legacy append profiles must not synthesize append opening preflight contracts",
        "fn kagemusha_recursive_spend_lineage_append_boundary_python_rejects_duplicate_current_outputs",
        ".push(profile.current_hop_statement.output_commitments[0])",
        "Python append-boundary helper must reject duplicate current-hop outputs",
        "fn kagemusha_recursive_spend_append_python_rejects_forged_previous_proof_opening_metadata",
        '"vk_commitment"',
        '"public_inputs_schema_hash"',
        '"domain_tag"',
        "fn kagemusha_recursive_spend_append_python_rejects_malformed_previous_proof_opening_archives",
        "Python host must reject {case}",
        "malformed previous-proof opening archive",
        "empty previous-proof opening vector",
        "over-count previous-proof opening vector",
        "fn kagemusha_recursive_spend_append_python_rejects_stale_previous_proof_payload_opening",
        "Python host must reject stale previous-proof payload opening",
        "fn kagemusha_recursive_spend_append_python_rejects_forged_previous_proof_circuit_id",
        "Python host must reject forged previous recursive proof circuit id",
        "forged previous proof circuit-id returned unexpected error",
        "fn kagemusha_recursive_spend_append_python_rejects_missing_lineage_key_artifacts",
        "Python host must reject missing Reserved-lineage key artifacts",
        "fn kagemusha_recursive_spend_init_python_rejects_forged_current_hop_pallas_metadata",
        "Python host must reject forged current-hop Pallas metadata",
        "current-hop metadata splice returned unexpected error",
        "fn kagemusha_recursive_spend_init_python_rejects_forged_current_hop_proof_circuit_id",
        "Python host must reject forged current-hop proof circuit id",
        "current-hop proof circuit-id splice returned unexpected error",
        "fn kagemusha_recursive_spend_init_python_rejects_missing_lineage_key_artifacts",
        "Python host must reject missing Reserved-lineage key artifacts",
        "fn kagemusha_recursive_spend_redeem_python_native_accepts_witnessless_reserved_lineage_public_binding",
        "witnessless reserved-lineage redeem validates before backend proof verification",
        "Python native redeem builder must reject backend-invalid reserved-lineage proof",
        "reserved-lineage redeem without verifier-slice columns must reject",
        "fn kagemusha_recursive_spend_redeem_python_function_rejects_structurally_invalid_lineage",
    ),
    "crates/iroha_data_model/benches/kagemusha_recursive_spend_payload.rs": (
        "kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight",
        "kagemusha_recursive_spend_accumulator_append_evidence_with_opening_preflight_contract",
        "kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight_contract",
        "kagemusha_recursive_spend_lineage_append_boundary_from_transition_profile",
        "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1",
        "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1",
        "previous_recursive_proof_open_envelopes_archive_digest",
        "append_opening_preflight_digest",
        "append transition profile benchmark must bind previous proof openings",
        "append transition profile benchmark must bind append opening preflight",
        "reserved-lineage benchmark accumulator must carry compact append boundary",
        "reserved-lineage recursive Kagemusha payload grew at hop",
    ),
}

SDK_HELPER_EDGE_COVERAGE = {
    "crates/iroha_data_model/src/offline/mod.rs": (
        "u32::MAX",
        "unknown-kagemusha-recursive-spend-circuit",
        "is_supported_kagemusha_recursive_spend_append_proof_transition",
        "Reserved-lineage to Reserved-lineage is the enabled structural append transition",
        "semantic previous proofs cannot select Reserved-lineage output",
        "preferred append selector falls back at the witnessless hop cap",
    ),
    "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift": (
        "LineageKeyArtifacts",
        "recursiveAggregationProofBackend",
        "isSupportedLineageKeyArtifactOpeningLen",
        "lineageKeyArtifactsForInit",
        "lineageKeyArtifactsForAppend",
        "validateLineageKeyArtifacts",
        "invalidLineageKeyArtifact",
        '"proof_circuit_id"',
        '"verifier_opening_len"',
        '"lineage_verifier_key"',
        '"lineage_proving_key_archive"',
        "requiresLineageKeyArtifactsForInit",
        "requiresLineageKeyArtifactsForAppendOutput",
        "normalizedAppendOutputCircuitId(outputCircuitId)",
    ),
    "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveSpendProverTests.swift": (
        "testLineageKeyArtifactPackagesValidateReleaseProfiles",
        "lineageKeyArtifactsForInit",
        "lineageKeyArtifactsForAppend",
        "validateLineageKeyArtifacts",
        "isSupportedLineageKeyArtifactOpeningLen(3)",
        "recursiveAggregationProofBackend",
        "halo2/kzg",
        "assertInvalidLineageKeyArtifact",
        "compactTokenMaxHops",
        "UInt32.max",
        "unknown-kagemusha-recursive-spend-circuit",
        "canAppendWitnesslessLineage(previousHopCount: UInt32.max)",
        "isSupportedAppendProofTransition",
        "requiresLineageKeyArtifactsForInit",
        "requiresLineageKeyArtifactsForAppendOutput",
        "outputCircuitId: nil",
        "semantic previous proofs cannot select Reserved-lineage output",
        "preferred append selector falls back at the witnessless hop cap",
    ),
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt": (
        "LineageKeyArtifacts",
        "RECURSIVE_AGGREGATION_PROOF_BACKEND",
        "isSupportedLineageKeyArtifactOpeningLen",
        "lineageKeyArtifactsForInit",
        "lineageKeyArtifactsForAppend",
        "validateLineageKeyArtifacts",
        '"proof_circuit_id"',
        '"verifier_opening_len"',
        '"lineage_verifier_key"',
        '"lineage_proving_key_archive"',
        "requiresLineageKeyArtifactsForInit",
        "requiresLineageKeyArtifactsForAppendOutput",
        "normalizeAppendOutputCircuitId(outputCircuitId)",
    ),
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProverTest.kt": (
        "lineageKeyArtifactPackagesValidateReleaseProfiles",
        "lineageKeyArtifactsForInit",
        "lineageKeyArtifactsForAppend",
        "validateLineageKeyArtifacts",
        "isSupportedLineageKeyArtifactOpeningLen(3)",
        "RECURSIVE_AGGREGATION_PROOF_BACKEND",
        "halo2/kzg",
        "assertContentEquals",
        "COMPACT_TOKEN_MAX_HOPS",
        "Int.MAX_VALUE",
        '"" to 1',
        "null to Int.MAX_VALUE",
        "canAppendWitnesslessLineage(-1)",
        "isSupportedAppendProofTransition",
        "requiresLineageKeyArtifactsForInit",
        "requiresLineageKeyArtifactsForAppendOutput",
        "listOf(",
        "semantic previous proofs cannot select Reserved-lineage output",
        "preferred append selector falls back at the witnessless hop cap",
    ),
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java": (
        "LineageKeyArtifacts",
        "RECURSIVE_AGGREGATION_PROOF_BACKEND",
        "isSupportedLineageKeyArtifactOpeningLen",
        "lineageKeyArtifactsForInit",
        "lineageKeyArtifactsForAppend",
        "validateLineageKeyArtifacts",
        '"proof_circuit_id"',
        '"verifier_opening_len"',
        '"lineage_verifier_key"',
        '"lineage_proving_key_archive"',
        "requiresLineageKeyArtifactsForInit",
        "requiresLineageKeyArtifactsForAppendOutput",
        "normalizeAppendOutputCircuitId(outputCircuitId)",
    ),
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProverTest.java": (
        "lineageKeyArtifactPackagesValidateReleaseProfiles",
        "lineageKeyArtifactsForInit",
        "lineageKeyArtifactsForAppend",
        "validateLineageKeyArtifacts",
        "isSupportedLineageKeyArtifactOpeningLen(3)",
        "RECURSIVE_AGGREGATION_PROOF_BACKEND",
        "halo2/kzg",
        "Arrays.equals",
        "COMPACT_TOKEN_MAX_HOPS",
        "Integer.MAX_VALUE",
        '{"", 1}',
        "{null, Integer.MAX_VALUE}",
        "canAppendWitnesslessLineage(-1)",
        "isSupportedAppendProofTransition",
        "requiresLineageKeyArtifactsForInit",
        "requiresLineageKeyArtifactsForAppendOutput(null)",
        "semantic previous proofs cannot select Reserved-lineage output",
        "preferred append selector falls back at the witnessless hop cap",
    ),
    "javascript/iroha_js/src/crypto.js": (
        "requiresKagemushaRecursiveSpendLineageKeyArtifactsForInit",
        "requiresKagemushaRecursiveSpendLineageKeyArtifactsForAppendOutput",
        "normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(outputProofCircuitId)",
    ),
    "javascript/iroha_js/dist/crypto.js": (
        "requiresKagemushaRecursiveSpendLineageKeyArtifactsForInit",
        "requiresKagemushaRecursiveSpendLineageKeyArtifactsForAppendOutput",
        "normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(outputProofCircuitId)",
    ),
    "javascript/iroha_js/src/crypto.browser.js": (
        "requiresKagemushaRecursiveSpendLineageKeyArtifactsForInit",
        "requiresKagemushaRecursiveSpendLineageKeyArtifactsForAppendOutput",
        "normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(outputProofCircuitId)",
    ),
    "javascript/iroha_js/dist/crypto.browser.js": (
        "requiresKagemushaRecursiveSpendLineageKeyArtifactsForInit",
        "requiresKagemushaRecursiveSpendLineageKeyArtifactsForAppendOutput",
        "normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(outputProofCircuitId)",
    ),
    "javascript/iroha_js/src/index.js": (
        "requiresKagemushaRecursiveSpendLineageKeyArtifactsForInit",
        "requiresKagemushaRecursiveSpendLineageKeyArtifactsForAppendOutput",
    ),
    "javascript/iroha_js/dist/index.js": (
        "requiresKagemushaRecursiveSpendLineageKeyArtifactsForInit",
        "requiresKagemushaRecursiveSpendLineageKeyArtifactsForAppendOutput",
    ),
    "javascript/iroha_js/index.d.ts": (
        "requiresKagemushaRecursiveSpendLineageKeyArtifactsForInit",
        "requiresKagemushaRecursiveSpendLineageKeyArtifactsForAppendOutput",
    ),
    "javascript/iroha_js/test/kagemushaRecursiveSpend.test.js": (
        "KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS",
        "Number.MAX_SAFE_INTEGER",
        "Number.NaN",
        "Number.POSITIVE_INFINITY",
        "canAppendKagemushaRecursiveSpendWitnesslessLineage(1n)",
        "canAppendKagemushaRecursiveSpendWitnesslessLineage(new Number(1))",
        "[undefined, 1]",
        "[null, 1]",
        "canAppendKagemushaRecursiveSpendWitnesslessLineage(true)",
        "isSupportedKagemushaRecursiveSpendAppendProofTransition",
        "kagemushaRecursiveSpendLineageAppendBoundary",
        "Kagemusha recursive spend transition profile append propagates forged opening rejection",
        "hop domain metadata mismatch",
        "requiresKagemushaRecursiveSpendLineageKeyArtifactsForInit",
        "requiresKagemushaRecursiveSpendLineageKeyArtifactsForAppendOutput",
        "undefined",
        "semantic previous proofs cannot select Reserved-lineage output",
        "preferred append selector falls back at the witnessless hop cap",
    ),
    "javascript/iroha_js/test/package_dist.test.js": (
        "KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS",
        "Number.MAX_SAFE_INTEGER",
        "Number.NaN",
        "Number.POSITIVE_INFINITY",
        "canAppendKagemushaRecursiveSpendWitnesslessLineage(1n)",
        "canAppendKagemushaRecursiveSpendWitnesslessLineage(new Number(1))",
        "[undefined, 1]",
        "[null, 1]",
        "canAppendKagemushaRecursiveSpendWitnesslessLineage(true)",
        "isSupportedKagemushaRecursiveSpendAppendProofTransition",
        "requiresKagemushaRecursiveSpendLineageKeyArtifactsForInit",
        "requiresKagemushaRecursiveSpendLineageKeyArtifactsForAppendOutput",
        "undefined",
        "semantic previous proofs cannot select Reserved-lineage output",
        "preferred append selector falls back at the witnessless hop cap",
    ),
    "python/iroha_python/src/iroha_python/kagemusha.py": (
        "requires_kagemusha_recursive_spend_lineage_key_artifacts_for_init",
        "requires_kagemusha_recursive_spend_lineage_key_artifacts_for_append_output",
        "normalize_kagemusha_recursive_spend_append_output_proof_circuit_id",
    ),
    "python/iroha_python/tests/kagemusha_test.py": (
        "KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS",
        "2**63",
        "1.5",
        'float("nan")',
        'float("inf")',
        'float("-inf")',
        "(None, 1)",
        "can_append_kagemusha_recursive_spend_witnessless_lineage(-1)",
        "is_supported_kagemusha_recursive_spend_append_proof_transition",
        "kagemusha_recursive_spend_lineage_append_boundary",
        "test_recursive_kagemusha_transition_profile_append_propagates_forged_opening_rejection",
        "hop domain metadata mismatch",
        "requires_kagemusha_recursive_spend_lineage_key_artifacts_for_init",
        "requires_kagemusha_recursive_spend_lineage_key_artifacts_for_append_output",
        "True,",
        "semantic previous proofs cannot select Reserved-lineage output",
        "preferred append selector falls back at the witnessless hop cap",
    ),
    "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs": (
        "RequiresLineageKeyArtifactsForInit",
        "RequiresLineageKeyArtifactsForAppendOutput",
        "NormalizeAppendOutputCircuitId(outputCircuitId)",
    ),
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/KagemushaRecursiveSpendNativeTests.cs": (
        "CompactTokenMaxHops",
        "uint.MaxValue",
        '("", 1u)',
        "(null, uint.MaxValue)",
        "CanAppendWitnesslessLineage(uint.MaxValue)",
        "IsSupportedAppendProofTransition",
        "RequiresLineageKeyArtifactsForInit",
        "RequiresLineageKeyArtifactsForAppendOutput(null)",
        "semantic previous proofs cannot select Reserved-lineage output",
        "preferred append selector falls back at the witnessless hop cap",
    ),
}
SDK_APPEND_CAP_BINDING_COVERAGE = {
    "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift": (
        "case recursiveAggregationProofCircuitIdV1:",
        "compactTokenMaxHops",
        "return previousHopCount < compactTokenMaxHops",
    ),
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt": (
        "RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 ->",
        "COMPACT_TOKEN_MAX_HOPS",
        "previousHopCount < COMPACT_TOKEN_MAX_HOPS",
    ),
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java": (
        "RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1.equals(normalized)",
        "COMPACT_TOKEN_MAX_HOPS",
        "return previousHopCount < COMPACT_TOKEN_MAX_HOPS;",
    ),
    "javascript/iroha_js/src/crypto.js": (
        "normalized === KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1",
        "KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS",
        "return previousHopCount < KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS;",
    ),
    "javascript/iroha_js/dist/crypto.js": (
        "normalized === KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1",
        "KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS",
        "return previousHopCount < KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS;",
    ),
    "javascript/iroha_js/src/crypto.browser.js": (
        "normalized === KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1",
        "KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS",
        "return previousHopCount < KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS;",
    ),
    "javascript/iroha_js/dist/crypto.browser.js": (
        "normalized === KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1",
        "KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS",
        "return previousHopCount < KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS;",
    ),
    "python/iroha_python/src/iroha_python/kagemusha.py": (
        "if normalized == KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1:",
        "KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS",
        "previous_hop_count < KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS",
    ),
    "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs": (
        "RecursiveAggregationProofCircuitIdV1 =>",
        "CompactTokenMaxHops",
        "previousHopCount < CompactTokenMaxHops",
    ),
}

NATIVE_OUTPUT_CAP_COVERAGE = {
    "crates/connect_norito_bridge/src/lib.rs": (
        "const KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES: usize = 64 * 1024 * 1024;",
        "fn kagemusha_archive_out_of_bounds(len: usize) -> bool",
        "len == 0 || len > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
        "unsafe fn write_kagemusha_archive_bridge",
        "clear_bridge_output(out_ptr, out_len);",
        "write_kagemusha_archive_bridge(out_bundle_ptr, out_bundle_len, &archive)",
        "fn kagemusha_native_archive_writer_rejects_empty_and_oversized_outputs",
        "KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES + 1",
    ),
    "IrohaSwift/Sources/IrohaSwift/KagemushaCompactPaymentTokenProver.swift": (
        "case invalidRecordBundleArchive",
        "case emptyRecordBundlePayload",
        "case oversizedCompactTokenArchive",
        "case invalidCompactTokenArchive",
        "case emptyCompactTokenPayload",
        "try requireValidRecordBundleArchive(recordBundleArchive)",
        "try requireValidCompactTokenArchive(token)",
        "noritoDecodeFrame(archive)",
        "Kagemusha verified fold record bundle archive must be a valid Norito archive.",
        "Kagemusha verified fold record bundle archive must contain a non-empty Norito payload.",
        "Kagemusha compact-token native bridge returned an invalid Norito archive.",
        "Kagemusha compact-token native bridge returned an empty Norito payload.",
    ),
    "IrohaSwift/Tests/IrohaSwiftTests/KagemushaCompactPaymentTokenProverTests.swift": (
        "testRejectsMalformedRecordBundleArchiveBeforeBridgeCall",
        "testRejectsEmptyPayloadRecordBundleArchiveBeforeBridgeCall",
        "testRejectsMalformedNativeOutput",
        "testRejectsEmptyPayloadNativeOutput",
        "testReturnsValidNativeOutput",
        ".invalidRecordBundleArchive",
        ".emptyRecordBundlePayload",
        ".invalidCompactTokenArchive",
        ".emptyCompactTokenPayload",
        "validKagemushaNoritoArchive",
        "emptyPayloadKagemushaNoritoArchive",
    ),
    "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveAggregationProofBundleProver.swift": (
        "case invalidRecordBundleArchive",
        "case emptyRecordBundlePayload",
        "case invalidPallasOpenEnvelopesArchive",
        "case emptyPallasOpenEnvelopesPayload",
        "case oversizedProofBundleArchive",
        "case invalidProofBundleArchive",
        "case emptyProofBundlePayload",
        "try requireValidInputArchive(",
        "try requireValidProofBundleArchive(proofBundle)",
        "noritoDecodeFrame(archive)",
        "Kagemusha verified fold record bundle archive must be a valid Norito archive.",
        "Kagemusha Pallas open-envelope archive must contain a non-empty Norito payload.",
        "Kagemusha recursive aggregation native bridge returned an invalid Norito archive.",
        "Kagemusha recursive aggregation native bridge returned an empty Norito payload.",
    ),
    "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveAggregationProofBundleProverTests.swift": (
        "testRejectsMalformedInputArchivesBeforeBridgeCall",
        "testRejectsEmptyPayloadInputArchivesBeforeBridgeCall",
        "testRejectsMalformedNativeOutput",
        "testRejectsEmptyPayloadNativeOutput",
        "testReturnsValidNativeOutput",
        ".invalidRecordBundleArchive",
        ".emptyPallasOpenEnvelopesPayload",
        ".invalidProofBundleArchive",
        ".emptyProofBundlePayload",
        "validKagemushaNoritoArchive",
        "emptyPayloadKagemushaNoritoArchive",
    ),
    "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift": (
        "case invalidInputArchive",
        "case emptyInputPayload",
        "case oversizedNativeOutput",
        "case invalidNativeOutput",
        "case emptyNativeOutputPayload",
        "public static let nativeArchiveMaxBytes = 64 * 1024 * 1024",
        "try archives.forEach(requireValidInputArchive)",
        "try requireValidOutputArchive(archive)",
        "noritoDecodeFrame(archive)",
        "Kagemusha recursive spend input archive must be a valid Norito archive.",
        "Kagemusha recursive spend input archive must contain a non-empty Norito payload.",
        "Kagemusha recursive spend native bridge returned an invalid Norito archive.",
        "Kagemusha recursive spend native bridge returned an empty Norito payload.",
        "guard archive.count <= nativeArchiveMaxBytes else",
    ),
    "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveSpendProverTests.swift": (
        "testRejectsMalformedInputArchivesBeforeBridgeCall",
        "testRejectsEmptyPayloadInputArchivesBeforeBridgeCall",
        "testRejectsMalformedNativeOutput",
        "testRejectsEmptyPayloadNativeOutput",
        "testReturnsValidNativeOutput",
        ".invalidInputArchive",
        ".emptyInputPayload",
        ".invalidNativeOutput",
        ".emptyNativeOutputPayload",
        "KagemushaRecursiveSpendProver.nativeArchiveMaxBytes, 64 * 1024 * 1024",
        "KagemushaRecursiveSpendProver.nativeArchiveMaxBytes + 1",
        "validKagemushaNoritoArchive",
        "emptyPayloadKagemushaNoritoArchive",
        ".oversizedNativeOutput",
    ),
    "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveCompactPaymentTokenProver.swift": (
        "case invalidRecordBundleArchive",
        "case emptyRecordBundlePayload",
        "case invalidPallasOpenEnvelopesArchive",
        "case emptyPallasOpenEnvelopesPayload",
        "try requireValidInputArchive(",
        "Kagemusha verified fold record bundle archive must be a valid Norito archive.",
        "Kagemusha Pallas open-envelope archive must contain a non-empty Norito payload.",
        "recursiveCompactUnavailable",
        "append verifier batch",
        "try requireValidRecursiveCompactTokenArchive(token)",
    ),
    "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveCompactPaymentTokenProverTests.swift": (
        "testRejectsMalformedInputArchivesBeforeBridgeCall",
        "testRejectsEmptyPayloadInputArchivesBeforeBridgeCall",
        ".invalidRecordBundleArchive",
        ".emptyPallasOpenEnvelopesPayload",
        "testNativeRecursiveCompactUnavailableIsDistinctFromProofRejection",
        "validKagemushaNoritoArchive",
        "emptyPayloadKagemushaNoritoArchive",
    ),
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaCompactPaymentTokenProver.java": (
        "public static final int NATIVE_ARCHIVE_MAX_BYTES = 64 * 1024 * 1024;",
        "requireNativeInput",
        "isValidNoritoArchive(archive)",
        "hasNonEmptyNoritoPayload(archive)",
        "must be a valid Norito archive",
        "must contain a non-empty Norito payload",
        "output.length > NATIVE_ARCHIVE_MAX_BYTES",
        "returned oversized output",
        "isValidNoritoArchive",
        "hasNonEmptyNoritoPayload",
        "CRC64_REFLECTED_POLY",
        "returned invalid Norito archive",
        "returned empty Norito payload",
    ),
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveAggregationProofBundleProver.java": (
        "proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes",
        "KagemushaCompactPaymentTokenProver.requireNativeInput",
        "recordBundleArchive",
        "pallasOpenEnvelopesArchive",
    ),
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/OfflineNoteTest.java": (
        "kagemushaRecordBackedNativeProverValidatesInput",
        "kagemushaRecursiveAggregationNativeProverValidatesInput",
        "recordBundleArchive must not be empty",
        "recordBundleArchive must be a valid Norito archive",
        "recordBundleArchive must contain a non-empty Norito payload",
        "pallasOpenEnvelopesArchive must not be empty",
        "pallasOpenEnvelopesArchive must be a valid Norito archive",
        "pallasOpenEnvelopesArchive must contain a non-empty Norito payload",
        "kagemushaNativeProversRejectMissingAndEmptyNativeOutputs",
        "returned invalid Norito archive",
        "returned empty Norito payload",
        "kagemushaNoritoFrameWithPayload",
    ),
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java": (
        "requireNativeInput",
        "requireRecursiveSpendOutput",
        "KagemushaCompactPaymentTokenProver.isValidNoritoArchive",
        "KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload",
        "must be a valid Norito archive",
        "must contain a non-empty Norito payload",
        "returned invalid Norito archive",
        "returned empty Norito payload",
    ),
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveCompactPaymentTokenProver.java": (
        "requireNativeInput",
        'requireNativeInput(recordBundleArchive, "recordBundleArchive")',
        'requireNativeInput(pallasOpenEnvelopesArchive, "pallasOpenEnvelopesArchive")',
        "KagemushaCompactPaymentTokenProver.isValidNoritoArchive(archive)",
        "KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload(archive)",
        "must be a valid Norito archive",
        "must contain a non-empty Norito payload",
    ),
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProverTest.java": (
        "rejectsMalformedAndEmptyPayloadArchivesBeforeNativeDispatch",
        "validRecursiveCompactInput",
        "recordBundleArchive must not be empty",
        "pallasOpenEnvelopesArchive must not be empty",
        "recordBundleArchive must be a valid Norito archive",
        "pallasOpenEnvelopesArchive must be a valid Norito archive",
        "recordBundleArchive must contain a non-empty Norito payload",
        "pallasOpenEnvelopesArchive must contain a non-empty Norito payload",
        "requestArchive must be a valid Norito archive",
        "previousWitnessArchive must contain a non-empty Norito payload",
        "KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES + 1",
        "native redeem returned oversized output",
        "native redeem returned invalid Norito archive",
        "native redeem returned empty Norito payload",
        "kagemushaNoritoFrameWithPayload",
    ),
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaCompactPaymentTokenProver.kt": (
        "const val NATIVE_ARCHIVE_MAX_BYTES: Int = 64 * 1024 * 1024",
        "requireNativeInput",
        "isValidNoritoArchive(archive)",
        "hasNonEmptyNoritoPayload(archive)",
        "must be a valid Norito archive",
        "must contain a non-empty Norito payload",
        "output.size <= NATIVE_ARCHIVE_MAX_BYTES",
        "returned oversized output",
        "isValidNoritoArchive",
        "hasNonEmptyNoritoPayload",
        "CRC64_REFLECTED_POLY",
        "returned invalid Norito archive",
        "returned empty Norito payload",
    ),
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveAggregationProofBundleProver.kt": (
        "proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes",
        "KagemushaCompactPaymentTokenProver.requireNativeInput",
        "recordBundleArchive",
        "pallasOpenEnvelopesArchive",
    ),
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/OfflineNoteTest.kt": (
        "kagemushaRecordBackedNativeProverValidatesInput",
        "kagemushaRecursiveAggregationNativeProverValidatesInput",
        "recordBundleArchive must not be empty",
        "recordBundleArchive must be a valid Norito archive",
        "recordBundleArchive must contain a non-empty Norito payload",
        "pallasOpenEnvelopesArchive must not be empty",
        "pallasOpenEnvelopesArchive must be a valid Norito archive",
        "pallasOpenEnvelopesArchive must contain a non-empty Norito payload",
        "kagemushaNativeProversRejectMissingAndEmptyNativeOutputs",
        "returned invalid Norito archive",
        "returned empty Norito payload",
        "kagemushaNoritoFrameWithPayload",
    ),
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt": (
        "requireNativeInput",
        "requireRecursiveSpendOutput",
        "KagemushaCompactPaymentTokenProver.isValidNoritoArchive",
        "KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload",
        "must be a valid Norito archive",
        "must contain a non-empty Norito payload",
        "returned invalid Norito archive",
        "returned empty Norito payload",
    ),
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveCompactPaymentTokenProver.kt": (
        "requireNativeInput",
        'requireNativeInput(recordBundleArchive, "recordBundleArchive")',
        'requireNativeInput(pallasOpenEnvelopesArchive, "pallasOpenEnvelopesArchive")',
        "KagemushaCompactPaymentTokenProver.isValidNoritoArchive(archive)",
        "KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload(archive)",
        "must be a valid Norito archive",
        "must contain a non-empty Norito payload",
    ),
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProverTest.kt": (
        "rejectsMalformedAndEmptyPayloadArchivesBeforeNativeDispatch",
        "validRecursiveCompactInput",
        "recordBundleArchive must not be empty",
        "pallasOpenEnvelopesArchive must not be empty",
        "recordBundleArchive must be a valid Norito archive",
        "pallasOpenEnvelopesArchive must be a valid Norito archive",
        "recordBundleArchive must contain a non-empty Norito payload",
        "pallasOpenEnvelopesArchive must contain a non-empty Norito payload",
        "requestArchive must be a valid Norito archive",
        "previousWitnessArchive must contain a non-empty Norito payload",
        "KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES + 1",
        "native redeem returned oversized output",
        "native redeem returned invalid Norito archive",
        "native redeem returned empty Norito payload",
        "kagemushaNoritoFrameWithPayload",
    ),
    "javascript/iroha_js/src/crypto.js": (
        "export const KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES = 64 * 1024 * 1024;",
        "toKagemushaArchiveView(value, name)",
        "toOwnedKagemushaArchiveBuffer(value, name)",
        "view.length > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
        "outputView.length > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
        "returned oversized output",
        "must not exceed",
        "const request = toOwnedKagemushaArchiveBuffer(requestArchive, archiveName)",
        'const recordBundle = toOwnedKagemushaArchiveBuffer(',
        'const compactToken = toOwnedKagemushaArchiveBuffer(',
        "assertKagemushaNoritoArchive(",
        "native ${operation} returned invalid Norito archive",
        "native ${operation} returned empty Norito payload",
    ),
    "javascript/iroha_js/dist/crypto.js": (
        "export const KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES = 64 * 1024 * 1024;",
        "toKagemushaArchiveView(value, name)",
        "toOwnedKagemushaArchiveBuffer(value, name)",
        "view.length > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
        "outputView.length > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
        "returned oversized output",
        "must not exceed",
        "const request = toOwnedKagemushaArchiveBuffer(requestArchive, archiveName)",
        'const recordBundle = toOwnedKagemushaArchiveBuffer(',
        'const compactToken = toOwnedKagemushaArchiveBuffer(',
        "assertKagemushaNoritoArchive(",
        "native ${operation} returned invalid Norito archive",
        "native ${operation} returned empty Norito payload",
    ),
    "javascript/iroha_js/src/crypto.browser.js": (
        "KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES = 64 * 1024 * 1024",
    ),
    "javascript/iroha_js/dist/crypto.browser.js": (
        "KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES = 64 * 1024 * 1024",
    ),
    "javascript/iroha_js/test/kagemushaRecursiveSpend.test.js": (
        "KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES, 64 * 1024 * 1024",
        "KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES + 1",
        "native kagemushaRecursiveSpendRedeem returned oversized output",
        "Kagemusha recursive spend helpers reject oversized request archives before native calls",
        "requestArchive must not exceed",
        "recordBundleArchive must not exceed",
        "pallasOpenEnvelopesArchive must not exceed",
        "previousWitnessArchive must not exceed",
        "compactTokenArchive must not exceed",
        "Kagemusha recursive spend helpers reject malformed Norito request archives before native calls",
        "Kagemusha recursive spend helpers reject empty-payload Norito request archives before native calls",
        "requestArchive must be a valid Norito archive",
        "recordBundleArchive must be a valid Norito archive",
        "pallasOpenEnvelopesArchive must contain a non-empty Norito payload",
        "previousWitnessArchive must contain a non-empty Norito payload",
        "kagemushaInputArchive",
        "Kagemusha recursive spend helpers reject malformed Norito native outputs",
        "Kagemusha recursive spend helpers reject empty-payload Norito native outputs",
        "native kagemushaRecursiveSpendRedeem returned invalid Norito archive",
        "native kagemushaRecursiveSpendRedeem returned empty Norito payload",
        "kagemushaNoritoFrameWithPayload",
    ),
    "javascript/iroha_js/test/package_dist.test.js": (
        "KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
        "64 * 1024 * 1024",
    ),
    "python/iroha_python/src/iroha_python/kagemusha.py": (
        "KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES = 64 * 1024 * 1024",
        '"KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES"',
        "_archive_bytes_named",
        "view = memoryview(archive)",
        "view.nbytes > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
        "return view.tobytes()",
        "view = memoryview(result)",
        "output = view.tobytes()",
        "returned oversized output",
        "_norito_archive_bytes_named",
        "_assert_kagemusha_norito_archive(data, name)",
        '_norito_archive_bytes_named(record_bundle_archive, "record_bundle_archive")',
        "pallas_open_envelopes = _norito_archive_bytes_named(",
        '_norito_archive_bytes_named(request_archive, "request_archive")',
        '_norito_archive_bytes_named(bundle_archive, "bundle_archive")',
        "_assert_kagemusha_norito_archive(output, name)",
        "returned invalid Norito archive",
        "returned empty Norito payload",
    ),
    "python/iroha_python/src/iroha_python/__init__.py": (
        "KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
    ),
    "python/iroha_python/tests/kagemusha_test.py": (
        "kagemusha.KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES == 64 * 1024 * 1024",
        "monkeypatch.setattr(kagemusha, \"KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES\", 48)",
        "test_recursive_kagemusha_helpers_reject_oversized_inputs_before_copy_and_native",
        "oversized Kagemusha input reached native loading",
        "test_recursive_kagemusha_helpers_reject_oversized_memoryview_native_outputs",
        "must not exceed",
        "compact_token_archive",
        "returned oversized output",
        "test_recursive_kagemusha_helpers_reject_malformed_norito_requests",
        "test_recursive_kagemusha_helpers_reject_empty_payload_norito_requests",
        "test_kagemusha_native_prover_helpers_reject_malformed_norito_requests",
        "test_kagemusha_native_prover_helpers_reject_empty_payload_norito_requests",
        "record_bundle_archive must be a valid Norito archive",
        "pallas_open_envelopes_archive must contain a non-empty Norito payload",
        "request_archive must be a valid Norito archive",
        "previous_witness_archive must contain a non-empty Norito payload",
        "_kagemusha_input_archive",
        "test_recursive_kagemusha_helpers_reject_malformed_native_outputs",
        "test_recursive_kagemusha_helpers_reject_empty_payload_native_outputs",
        "returned invalid Norito archive",
        "returned empty Norito payload",
        "_kagemusha_norito_frame_with_payload",
    ),
    "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs": (
        "public const int NativeArchiveMaxBytes = 64 * 1024 * 1024;",
        "rawLength > NativeArchiveMaxBytes",
        "RequireValidInputArchive",
        "Request archive",
        "Bundle archive",
        "Record bundle archive",
        "Pallas open-envelopes archive",
        "archive.Length > NativeArchiveMaxBytes",
        "compactTokenArchive.Length > NativeArchiveMaxBytes",
        "must not exceed",
        "must be a valid Norito archive.",
        "must contain a non-empty Norito payload.",
        "PrivacyNative.IsNoritoV1Archive(bytes)",
        "PrivacyNative.HasNonEmptyPrivacyNoritoPayload(bytes)",
        "returned oversized output",
        "RequireValidNativeOutput(symbol, result)",
        "PrivacyNative.IsNoritoV1Archive",
        "returned invalid Norito archive",
        "returned empty Norito payload",
    ),
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/KagemushaRecursiveSpendNativeTests.cs": (
        "KagemushaRecursiveSpendNative.NativeArchiveMaxBytes",
        "KagemushaRecursiveSpendNative.NativeArchiveMaxBytes + 1UL",
        "oversized output",
        "RecursiveSpendNativeRejectsMalformedArchivesBeforeLoadingNativeBridge",
        "RecursiveSpendNativeRejectsOversizedArchivesBeforeLoadingNativeBridge",
        "RecursiveSpendNativeRejectsEmptyPayloadArchivesBeforeLoadingNativeBridge",
        "RecursiveCompactProverRejectsMalformedInputsBeforeLoadingNativeBridge",
        "RecursiveCompactProverRejectsOversizedInputsBeforeLoadingNativeBridge",
        "RecursiveCompactProverRejectsEmptyPayloadInputsBeforeLoadingNativeBridge",
        "CompactTokenProverRejectsOversizedInputsBeforeLoadingNativeBridge",
        "RecursiveAggregationProverRejectsOversizedInputsBeforeLoadingNativeBridge",
        "RecursiveCompactVerifierRejectsOversizedInputBeforeLoadingNativeBridge",
        "Previous witness archive must not exceed",
        "Compact token archive must not exceed",
        "Record bundle archive must be a valid Norito archive",
        "Pallas open-envelopes archive must contain a non-empty Norito payload",
        "RecursiveSpendNativeReadBridgeOutputRejectsMalformedNoritoSuccessOutput",
        "RecursiveSpendNativeReadBridgeOutputRejectsEmptyPayloadNoritoSuccessOutput",
        "RecursiveSpendNativeReadBridgeOutputReturnsValidNoritoSuccessOutput",
        "KagemushaNoritoFrameWithPayload",
    ),
}

RESERVED_LINEAGE_PROFILE_SPLIT_COVERAGE = {
    "crates/iroha_data_model/src/offline/mod.rs": (
        "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1",
        "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1",
        "is_kagemusha_recursive_spend_lineage_proof_circuit_id",
        "is_kagemusha_recursive_spend_lineage_append_output_circuit_id",
        "normalize_kagemusha_recursive_spend_append_output_proof_circuit_id",
        "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1",
        "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1",
    ),
    "crates/iroha_core/src/zk.rs": (
        "pub const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_CIRCUIT_ID",
        "pub const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_CIRCUIT_ID",
        "kagemusha_recursive_spend_lineage_vk_record_from_box",
        "kagemusha_recursive_spend_lineage_append_vk_record_from_box",
        "kagemusha_recursive_spend_lineage_append_vk_record",
        "derive_halo2_ipa_kagemusha_recursive_spend_lineage_one_hop_proving_key_bytes_from_pallas_open_envelope_archive",
        "derive_halo2_ipa_kagemusha_recursive_spend_lineage_append_proving_key_bytes_from_pallas_open_envelope_archive",
        "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_CIRCUIT_ID",
        "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_CIRCUIT_ID",
        "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_KEY_ARTIFACTS_REQUIRED",
        "lineage_proving_key_archive_helpers_reject_profile_mismatch_and_malformed_inputs",
        "lineage_vk_record_from_box_canonicalizes_profiles_without_keygen",
        "kagemusha_recursive_spend_lineage_init_default_rejects_missing_key_artifacts_before_runtime_keygen",
        "fn kagemusha_recursive_fixed_window_shared_table_manifest_digest_rejects_layout_splices",
        "kagemusha_recursive_fixed_window_shared_table_manifest_digest_from_parts",
        "manifest schedule digest splice must change digest",
        "manifest row splice must change digest",
        "manifest row-count splice must change digest",
        "manifest role splice must change digest",
        "manifest family-count splice must change digest",
        "Reserved-lineage one-hop and append verifier records must coexist under distinct circuit ids",
        '"halo2/pasta/kagemusha-recursive-spend-lineage-onehop-v1"',
        '"halo2/pasta/kagemusha-recursive-spend-lineage-append-v1"',
    ),
    "crates/iroha_cli/src/zk.rs": (
        "KagemushaCommand",
        "LineageKeyArtifacts",
        "RecursiveCompactKeyArtifacts",
        "LineageRecord",
        "KagemushaRecursiveCompactKeyArtifactsArgs",
        "KagemushaLineageRecordArgs",
        "kagemusha_recursive_compact_vk_record_from_bytes",
        "kagemusha_lineage_vk_record_from_bytes",
        "std::fs::read(&self.vk)",
        "kagemusha_recursive_compact_record_from_existing_vk_bytes_rejects_adversarial_inputs",
        "kagemusha_lineage_record_run_writes_norito_record_from_existing_vk_file",
        "record_out",
        "record_namespace",
        "record_version",
        "kagemusha_recursive_compact_payment_token_vk_record_from_box",
        "kagemusha_recursive_spend_lineage_vk_record_from_box",
        "kagemusha_recursive_spend_lineage_append_vk_record_from_box",
        '"offline_kagemusha"',
    ),
    "docs/source/offline_kagemusha.md": (
        "--record-out",
        "lineage-record",
        "recursive-compact-key-artifacts",
        "recursive-compact-len4.pk",
        "--vk artifacts/kagemusha/lineage-init-len128.vk",
        "--vk artifacts/kagemusha/lineage-append-len128.vk",
        "--record-namespace",
        "--record-version",
        "VerifyingKeyRecord",
    ),
    "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift": (
        "recursiveSpendLineageOneHopProofCircuitIdV1",
        "recursiveSpendLineageAppendProofCircuitIdV1",
        "isLineageProofCircuitId",
        "isLineageAppendOutputCircuitId",
    ),
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt": (
        "RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1",
        "RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1",
        "isLineageProofCircuitId",
        "isLineageAppendOutputCircuitId",
    ),
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java": (
        "RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1",
        "RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1",
        "isLineageProofCircuitId",
        "isLineageAppendOutputCircuitId",
    ),
    "javascript/iroha_js/src/crypto.js": (
        "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1",
        "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1",
        "isKagemushaRecursiveSpendLineageProofCircuitId",
        "isKagemushaRecursiveSpendLineageAppendOutputCircuitId",
    ),
    "javascript/iroha_js/dist/crypto.js": (
        "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1",
        "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1",
        "isKagemushaRecursiveSpendLineageProofCircuitId",
        "isKagemushaRecursiveSpendLineageAppendOutputCircuitId",
    ),
    "python/iroha_python/src/iroha_python/kagemusha.py": (
        "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1",
        "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1",
        "is_kagemusha_recursive_spend_lineage_proof_circuit_id",
        "is_kagemusha_recursive_spend_lineage_append_output_circuit_id",
    ),
    "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs": (
        "RecursiveSpendLineageOneHopProofCircuitIdV1",
        "RecursiveSpendLineageAppendProofCircuitIdV1",
        "IsLineageProofCircuitId",
        "IsLineageAppendOutputCircuitId",
    ),
}

VERIFY_RESULT_FAIL_CLOSED_COVERAGE = {
    "crates/iroha_core/src/zk.rs": (
        "let witnessless_redeem_supported =",
        "can_redeem_kagemusha_recursive_spend_witnessless(",
        "&bundle.recursive_proof.verifier_key_id.name",
        "bundle.accumulator.hop_count",
        "let lineage_witness_required_for_redeem = !witnessless_redeem_supported;",
        "witnessless_redeem_supported: false,",
        "lineage_witness_required_for_redeem: true,",
        "assert!(!semantic_with_record.witnessless_redeem_supported);",
        "assert!(semantic_with_record.lineage_witness_required_for_redeem);",
        "assert!(!missing_record.witnessless_redeem_supported);",
        "assert!(missing_record.lineage_witness_required_for_redeem);",
        "assert!(!wrong_record.witnessless_redeem_supported);",
        "assert!(wrong_record.lineage_witness_required_for_redeem);",
        "assert!(!wrong_multi_hop.witnessless_redeem_supported);",
        "assert!(wrong_multi_hop.lineage_witness_required_for_redeem);",
        "can_redeem_kagemusha_recursive_spend_witnessless(",
        "metadata-valid two-hop append lineage profile must remain witnessless-redeem capable",
    ),
    "crates/connect_norito_bridge/src/lib.rs": (
        "assert!(!result.witnessless_redeem_supported);",
        "assert!(result.lineage_witness_required_for_redeem);",
        "fn assert_request_rejected(request: &KagemushaRecursiveSpendVerifyRequestV1)",
        "malformed verify request must not return a diagnostic archive",
        "assert_request_rejected(&trusted_setup_backend);",
        "assert_request_rejected(&stark_recursive_bundle);",
        "assert_request_rejected(&empty_recursive_proof);",
    ),
    "crates/iroha_js_host/src/lib.rs": (
        "assert!(!no_height.witnessless_redeem_supported);",
        "assert!(no_height.lineage_witness_required_for_redeem);",
    ),
    "python/iroha_python/iroha_python_rs/src/lib.rs": (
        "fn expect_request_error(",
        "malformed verify request must reject",
        "expect_request_error(py, &trusted_setup_backend, \"proof.backend\");",
        "expect_request_error(py, &stark_recursive_bundle, \"proof.backend\");",
        "expect_request_error(py, &empty_recursive_proof, \"proof.bytes\");",
        "assert!(!result.witnessless_redeem_supported);",
        "assert!(result.lineage_witness_required_for_redeem);",
    ),
}

WORKFLOW_PATH = ".github/workflows/pr_kagemusha_payload_bench.yml"
CI_GUARD_PATHS = (
    "ci/check_connect_norito_bridge_header.sh",
    "ci/check_kagemusha_recursive_spend_policy.sh",
    "ci/check_kagemusha_recursive_spend_payload_bench.sh",
    "ci/check_kagemusha_recursive_spend_sdk_parity.sh",
    "ci/check_kagemusha_recursive_spend_python_sdk.sh",
)
PAYLOAD_BENCH_REQUIRED_PATHS = (
    "crates/iroha_data_model/Cargo.toml",
    "crates/iroha_data_model/benches/kagemusha_recursive_spend_payload.rs",
)
PAYLOAD_BENCH_SOURCE_COVERAGE = {
    "crates/iroha_data_model/benches/kagemusha_recursive_spend_payload.rs": (
        "kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight",
        "kagemusha_recursive_spend_accumulator_append_evidence_with_opening_preflight_contract",
        "kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight_contract",
        "kagemusha_recursive_spend_lineage_append_boundary_from_transition_profile",
        "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1",
        "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1",
        "previous_recursive_proof_open_envelopes_archive_digest",
        "append_opening_preflight_digest",
        "previous_proof_open_envelope_archive",
        "kagemusha_recursive_spend_payload_bytes",
        "kagemusha_recursive_spend_transition_profile_bytes",
        "kagemusha_recursive_spend_reserved_lineage_payload_bytes",
        "kagemusha_reserved_lineage_transition_profile_bytes",
        "recursive Kagemusha payload grew at hop",
        "recursive Kagemusha append transition profile grew at hop",
        "reserved-lineage recursive Kagemusha payload grew at hop",
        "reserved-lineage recursive Kagemusha append transition profile grew at hop",
    ),
}
WORKFLOW_REQUIRED_PATHS = (
    WORKFLOW_PATH,
    SHARED_FIXTURE_PATH,
    *CI_GUARD_PATHS,
    *PAYLOAD_BENCH_REQUIRED_PATHS,
    *DOC_PATHS,
    *SHARED_FIXTURE_COVERAGE.keys(),
    *ADVERSARIAL_COVERAGE.keys(),
    *SDK_HELPER_EDGE_COVERAGE.keys(),
    *SDK_APPEND_CAP_BINDING_COVERAGE.keys(),
    *NATIVE_OUTPUT_CAP_COVERAGE.keys(),
    *RESERVED_LINEAGE_PROFILE_SPLIT_COVERAGE.keys(),
    *VERIFY_RESULT_FAIL_CLOSED_COVERAGE.keys(),
)
WORKFLOW_MAIN_GUARD_COMMANDS = (
    (
        "NoritoBridge recursive spend header parity",
        "ci/check_connect_norito_bridge_header.sh",
    ),
    (
        "Kagemusha recursive spend SDK parity",
        "ci/check_kagemusha_recursive_spend_sdk_parity.sh",
    ),
    (
        "Kagemusha recursive spend Reserved-lineage policy",
        "ci/check_kagemusha_recursive_spend_policy.sh",
    ),
)
HEADER_NEGATIVE_CONTROL_COMMANDS = (
    (
        "missing recursive header declaration negative control",
        "ci/check_connect_norito_bridge_header.sh --negative-control-missing-recursive-header",
    ),
    (
        "bad recursive header signature negative control",
        "ci/check_connect_norito_bridge_header.sh --negative-control-bad-recursive-signature",
    ),
    (
        "missing Rust export negative control",
        "ci/check_connect_norito_bridge_header.sh --negative-control-missing-rust-export",
    ),
    (
        "umbrella header drift negative control",
        "ci/check_connect_norito_bridge_header.sh --negative-control-umbrella-drift",
    ),
)
POLICY_MAIN_COMMAND = "ci/check_kagemusha_recursive_spend_policy.sh"
PYTHON_SDK_TEST_COMMAND = "ci/check_kagemusha_recursive_spend_python_sdk.sh"
POLICY_NEGATIVE_CONTROL_COMMANDS = (
    (
        "SDK helper edge-case negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control",
    ),
    (
        "SDK selector edge-case negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-sdk-selector-edge",
    ),
    (
        "SDK preferred cap edge-case negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-sdk-preferred-cap-edge",
    ),
    (
        "JavaScript package-dist selector edge-case negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-js-package-dist-selector-edge",
    ),
    (
        "Python SDK non-finite hop-count negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-python-hop-edges",
    ),
    (
        "JavaScript source SDK BigInt hop-count negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-js-hop-edges",
    ),
    (
        "JavaScript package-dist SDK BigInt hop-count negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-js-package-dist-hop-edges",
    ),
    (
        "SDK append cap constant-binding negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-sdk-append-cap-binding",
    ),
    (
        "native output cap negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-native-output-cap",
    ),
    (
        "shared ABI-6 fixture manifest negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-shared-fixture-manifest",
    ),
    (
        "shared ABI-6 archive fixture negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-shared-archive-fixture",
    ),
    (
        "data-model append cap request-boundary negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-data-model-append-cap-boundary",
    ),
    (
        "data-model self-consistent forged append-boundary negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-data-model-self-consistent-boundary",
    ),
    (
        "data-model transition-profile current-hop set negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-data-model-transition-profile-current-hop-sets",
    ),
    (
        "core append cap direct-prover negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-append-cap-boundary",
    ),
    (
        "data-model lineage key package-binding negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-data-model-lineage-key-package-binding",
    ),
    (
        "Reserved-lineage profile split negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-lineage-profile-split",
    ),
    (
        "core recursive spend proof-chain accumulator negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-proof-chain-accumulator",
    ),
    (
        "core non-native Vesta IPA H-fold negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-vesta-ipa-h-fold",
    ),
    (
        "core non-native Vesta IPA G-fold negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-vesta-ipa-g-fold",
    ),
    (
        "core append opening-preflight splice negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-opening-preflight-splices",
    ),
    (
        "current-hop opening metadata splice negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-current-hop-opening-metadata-splice",
    ),
    (
        "append verifier-slice Pallas preflight binding negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-append-verifier-slice-preflight-binding",
    ),
    (
        "one-hop verifier-slice evidence binding negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-one-hop-verifier-slice-evidence-binding",
    ),
    (
        "core checked-fold input/output overlap negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-fold-overlap-predecode",
    ),
    (
        "core lineage witness fold metadata predecode negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-lineage-witness-fold-predecode",
    ),
    (
        "core lineage witness verifier-record predecode negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-lineage-witness-record-predecode",
    ),
    (
        "core lineage witness current-note predecode negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-lineage-witness-note-predecode",
    ),
    (
        "core lineage witness current-note binding predecode negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-lineage-witness-note-binding-predecode",
    ),
    (
        "core lineage witness append-handoff predecode negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-lineage-witness-handoff-predecode",
    ),
    (
        "core lineage witness final-bundle predecode negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-lineage-witness-final-bundle-predecode",
    ),
    (
        "recursive compact public instance shape negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-recursive-compact-public-instance-shape",
    ),
    (
        "recursive compact Pallas opening count negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-recursive-compact-pallas-count",
    ),
    (
        "recursive compact Pallas metadata negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-recursive-compact-pallas-metadata",
    ),
    (
        "recursive compact CID-spoof verifier-key negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-recursive-compact-cid-spoof-key",
    ),
    (
        "recursive spend compact projection token negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-recursive-spend-compact-projection-token",
    ),
    (
        "bridge recursive compact public instance shape negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-bridge-recursive-compact-public-instance-shape",
    ),
    (
        "bridge recursive compact Pallas opening count negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-bridge-recursive-compact-pallas-count",
    ),
    (
        "bridge recursive compact Pallas metadata negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-bridge-recursive-compact-pallas-metadata",
    ),
    (
        "bridge recursive compact verifier-key hash negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-bridge-recursive-compact-vk-hash",
    ),
    (
        "JS host recursive compact verifier-key hash negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-js-host-recursive-compact-vk-hash",
    ),
    (
        "JS host recursive compact Pallas opening count negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-js-host-recursive-compact-pallas-count",
    ),
    (
        "JS host recursive compact Pallas metadata negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-js-host-recursive-compact-pallas-metadata",
    ),
    (
        "JS host recursive compact public instance shape negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-js-host-recursive-compact-public-instance-shape",
    ),
    (
        "JS host Kagemusha archive cap negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-js-host-kagemusha-archive-cap",
    ),
    (
        "JS host append-boundary current-hop output-set negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-js-host-append-boundary-current-output-set",
    ),
    (
        "Python recursive compact verifier-key hash negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-python-recursive-compact-vk-hash",
    ),
    (
        "Python recursive compact Pallas opening count negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-python-recursive-compact-pallas-count",
    ),
    (
        "Python recursive compact Pallas metadata negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-python-recursive-compact-pallas-metadata",
    ),
    (
        "Python recursive compact public instance shape negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-python-recursive-compact-public-instance-shape",
    ),
    (
        "Python Kagemusha archive cap negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-python-kagemusha-archive-cap",
    ),
    (
        "Python append-boundary current-hop output-set negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-python-append-boundary-current-output-set",
    ),
    (
        "fixed-window manifest digest splice negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-fixed-window-manifest-digest-splice",
    ),
    (
        "workflow path negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-workflow",
    ),
    (
        "JavaScript package-dist workflow path negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-js-package-dist-workflow",
    ),
    (
        "core ISI workflow path negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-isi-workflow",
    ),
    (
        "payload reducer script workflow path negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-payload-script-workflow",
    ),
    (
        "CI guard script workflow path negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-ci-guard-script-workflow",
    ),
    (
        "payload reducer self-test workflow negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-payload-self-test-workflow",
    ),
    (
        "payload reducer self-test ordering negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-payload-self-test-order-workflow",
    ),
    (
        "payload reducer missing-payload workflow negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-payload-missing-payload-workflow",
    ),
    (
        "payload reducer negative-control workflow negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-payload-negative-controls-workflow",
    ),
    (
        "Reserved-lineage payload reducer negative-control workflow negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-reserved-lineage-payload-negative-controls-workflow",
    ),
    (
        "payload reducer commented-command negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-payload-negative-controls-comment-workflow",
    ),
    (
        "payload reducer negative-control ordering negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-payload-negative-controls-order-workflow",
    ),
    (
        "payload benchmark manifest workflow negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-payload-benchmark-manifest-workflow",
    ),
    (
        "payload benchmark source workflow negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-payload-benchmark-workflow",
    ),
    (
        "payload benchmark source coverage negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-payload-benchmark-source",
    ),
    (
        "documentation payload-budget negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-doc-payload-budget",
    ),
    (
        "documentation SDK host-boundary negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-doc-sdk-host-boundary",
    ),
    (
        "documentation SDK availability surface negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-doc-sdk-availability-surface",
    ),
    (
        "documentation ABI-6 entry-count negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-doc-abi-entry-count",
    ),
    (
        "roadmap ABI-6 complete-surface negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-roadmap-abi-surface",
    ),
    (
        "core ISI coverage negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-isi",
    ),
    (
        "core multi-hop redeem success negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-multi-hop-redeem-success",
    ),
    (
        "core malformed lineage hop proof negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-lineage-hop-proof",
    ),
    (
        "core redeem execution-order negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-redeem-order",
    ),
    (
        "early core redeem mint negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-redeem-early-mint",
    ),
    (
        "verify-result flag negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-verify-result-flags",
    ),
    (
        "status documentation drift negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-status-doc-drift",
    ),
    (
        "workflow cancellation negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-workflow-cancel-in-progress",
    ),
    (
        "main guard workflow negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-main-guards-workflow",
    ),
    (
        "policy negative-control workflow negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-policy-negative-controls-workflow",
    ),
    (
        "policy commented-command workflow negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-policy-negative-controls-comment-workflow",
    ),
    (
        "Python SDK test workflow negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-python-sdk-test-workflow",
    ),
    (
        "policy negative-control ordering workflow negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-policy-negative-controls-order-workflow",
    ),
    (
        "header negative-control workflow negative control",
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-header-negative-controls-workflow",
    ),
)


class PolicyError(RuntimeError):
    pass


def fail(message):
    raise PolicyError(message)


def read(relative):
    if relative in text_overrides:
        return text_overrides[relative]
    return (root / relative).read_text(encoding="utf-8")


def workflow_trigger_paths():
    paths = []
    in_paths = False
    for line in read(WORKFLOW_PATH).splitlines():
        if line.strip() == "paths:":
            in_paths = True
            continue
        if not in_paths:
            continue
        match = re.match(r'\s+-\s+"([^"]+)"\s*$', line)
        if match is not None:
            paths.append(match.group(1))
            continue
        if line and not line.startswith("      "):
            break
    return paths


def check_workflow_paths_cover_policy_sources():
    trigger_paths = workflow_trigger_paths()
    missing = sorted({
        relative
        for relative in WORKFLOW_REQUIRED_PATHS
        if not any(fnmatchcase(relative, pattern) for pattern in trigger_paths)
    })
    if missing:
        fail(
            "Kagemusha payload workflow paths do not cover fail-closed policy sources: "
            + ", ".join(missing)
        )


def check_workflow_preserves_in_progress_runs():
    workflow = read(WORKFLOW_PATH)
    if re.search(r"(?m)^\s*cancel-in-progress:\s*true\s*$", workflow) is not None:
        fail(
            "Kagemusha payload workflow must not cancel in-progress runs; "
            "long proof/benchmark evidence must be allowed to finish"
        )
    if re.search(r"(?m)^\s*cancel-in-progress:\s*false\s*$", workflow) is None:
        fail(
            "Kagemusha payload workflow must not cancel in-progress runs; "
            "long proof/benchmark evidence must be allowed to finish"
        )


def workflow_real_benchmark_index(workflow):
    benchmark_match = re.search(
        r"^\s+run:\s+ci/check_kagemusha_recursive_spend_payload_bench\.sh\s*$",
        workflow,
        re.M,
    )
    if benchmark_match is None:
        fail("Kagemusha payload workflow must run the real payload benchmark")
    return benchmark_match.start()


def workflow_command_match(workflow, command):
    pattern = rf"(?m)^\s+(?:run:\s+)?{re.escape(command)}\s*$"
    return re.search(pattern, workflow)


def workflow_policy_main_guard_index(workflow):
    match = re.search(rf"(?m)^\s+run:\s+{re.escape(POLICY_MAIN_COMMAND)}\s*$", workflow)
    if match is None:
        fail("Kagemusha payload workflow must run the main Kagemusha recursive spend fail-closed policy guard")
    return match.start()


def check_workflow_runs_main_guards():
    workflow = read(WORKFLOW_PATH)
    benchmark_index = workflow_real_benchmark_index(workflow)
    for label, command in WORKFLOW_MAIN_GUARD_COMMANDS:
        match = re.search(rf"(?m)^\s+run:\s+{re.escape(command)}\s*$", workflow)
        if match is None:
            fail(f"Kagemusha payload workflow must run the main {label} guard")
        if match.start() > benchmark_index:
            fail(f"Kagemusha payload workflow must run the main {label} guard before the real benchmark")


def check_workflow_runs_header_negative_controls():
    workflow = read(WORKFLOW_PATH)
    benchmark_index = workflow_real_benchmark_index(workflow)
    for label, command in HEADER_NEGATIVE_CONTROL_COMMANDS:
        match = workflow_command_match(workflow, command)
        if match is None:
            fail(f"Kagemusha payload workflow must run the NoritoBridge {label}")
        if match.start() > benchmark_index:
            fail(f"Kagemusha payload workflow must run the NoritoBridge {label} before the real benchmark")


def check_workflow_runs_payload_reducer_controls():
    workflow = read(WORKFLOW_PATH)
    benchmark_index = workflow_real_benchmark_index(workflow)
    required_before_benchmark = (
        (
            "payload reducer self-test",
            "ci/check_kagemusha_recursive_spend_payload_bench.sh --self-test",
        ),
        (
            "payload baseline negative control",
            "ci/check_kagemusha_recursive_spend_payload_bench.sh --negative-control-payload-baseline",
        ),
        (
            "payload growth negative control",
            "ci/check_kagemusha_recursive_spend_payload_bench.sh --negative-control-payload-growth",
        ),
        (
            "missing payload negative control",
            "ci/check_kagemusha_recursive_spend_payload_bench.sh --negative-control-missing-payload",
        ),
        (
            "transition-profile growth negative control",
            "ci/check_kagemusha_recursive_spend_payload_bench.sh --negative-control-transition-profile-growth",
        ),
        (
            "transition-profile baseline negative control",
            "ci/check_kagemusha_recursive_spend_payload_bench.sh --negative-control-transition-profile-baseline",
        ),
        (
            "missing transition-profile negative control",
            "ci/check_kagemusha_recursive_spend_payload_bench.sh --negative-control-missing-transition-profile",
        ),
        (
            "Reserved-lineage payload baseline negative control",
            "ci/check_kagemusha_recursive_spend_payload_bench.sh --negative-control-reserved-lineage-payload-baseline",
        ),
        (
            "Reserved-lineage payload growth negative control",
            "ci/check_kagemusha_recursive_spend_payload_bench.sh --negative-control-reserved-lineage-payload-growth",
        ),
        (
            "missing Reserved-lineage payload negative control",
            "ci/check_kagemusha_recursive_spend_payload_bench.sh --negative-control-missing-reserved-lineage-payload",
        ),
        (
            "Reserved-lineage transition-profile baseline negative control",
            "ci/check_kagemusha_recursive_spend_payload_bench.sh --negative-control-reserved-lineage-transition-profile-baseline",
        ),
        (
            "Reserved-lineage transition-profile growth negative control",
            "ci/check_kagemusha_recursive_spend_payload_bench.sh --negative-control-reserved-lineage-transition-profile-growth",
        ),
        (
            "missing Reserved-lineage transition-profile negative control",
            "ci/check_kagemusha_recursive_spend_payload_bench.sh --negative-control-missing-reserved-lineage-transition-profile",
        ),
        (
            "unexpected payload-hop negative control",
            "ci/check_kagemusha_recursive_spend_payload_bench.sh --negative-control-unexpected-payload-hop",
        ),
        (
            "unexpected transition-profile-hop negative control",
            "ci/check_kagemusha_recursive_spend_payload_bench.sh --negative-control-unexpected-transition-profile-hop",
        ),
        (
            "unexpected Reserved-lineage payload-hop negative control",
            "ci/check_kagemusha_recursive_spend_payload_bench.sh --negative-control-unexpected-reserved-lineage-payload-hop",
        ),
        (
            "unexpected Reserved-lineage transition-profile-hop negative control",
            "ci/check_kagemusha_recursive_spend_payload_bench.sh --negative-control-unexpected-reserved-lineage-transition-profile-hop",
        ),
        (
            "conflicting payload-size negative control",
            "ci/check_kagemusha_recursive_spend_payload_bench.sh --negative-control-conflicting-payload-size",
        ),
        (
            "conflicting transition-profile-size negative control",
            "ci/check_kagemusha_recursive_spend_payload_bench.sh --negative-control-conflicting-transition-profile-size",
        ),
        (
            "conflicting Reserved-lineage payload-size negative control",
            "ci/check_kagemusha_recursive_spend_payload_bench.sh --negative-control-conflicting-reserved-lineage-payload-size",
        ),
        (
            "conflicting Reserved-lineage transition-profile-size negative control",
            "ci/check_kagemusha_recursive_spend_payload_bench.sh --negative-control-conflicting-reserved-lineage-transition-profile-size",
        ),
    )
    for label, command in required_before_benchmark:
        match = workflow_command_match(workflow, command)
        if match is None:
            fail(f"Kagemusha payload workflow must run the {label} before benchmarking")
        if match.start() > benchmark_index:
            fail(f"Kagemusha payload workflow must run the {label} before the real benchmark")


def check_workflow_runs_policy_negative_controls():
    workflow = read(WORKFLOW_PATH)
    benchmark_index = workflow_real_benchmark_index(workflow)
    policy_main_index = workflow_policy_main_guard_index(workflow)
    for label, command in POLICY_NEGATIVE_CONTROL_COMMANDS:
        match = workflow_command_match(workflow, command)
        if match is None:
            fail(f"Kagemusha payload workflow must run the policy {label}")
        if match.start() > benchmark_index:
            fail(f"Kagemusha payload workflow must run the policy {label} before the real benchmark")
        if match.start() > policy_main_index:
            fail(f"Kagemusha payload workflow must run the policy {label} before the main policy guard")


def check_workflow_runs_python_sdk_tests():
    workflow = read(WORKFLOW_PATH)
    benchmark_index = workflow_real_benchmark_index(workflow)
    match = workflow_command_match(workflow, PYTHON_SDK_TEST_COMMAND)
    if match is None:
        fail("Kagemusha payload workflow must run the Python recursive spend SDK tests before benchmarking")
    if match.start() > benchmark_index:
        fail("Kagemusha payload workflow must run the Python recursive spend SDK tests before the real benchmark")


def check_core_redeem_execution_order():
    rust = read("crates/iroha_core/src/smartcontracts/isi/offline.rs")
    match = re.search(
        r"impl\s+Execute\s+for\s+RedeemKagemushaRecursive\s*\{(?P<body>.*?)\n\s*}\n\s*\n\s*#\[cfg\(test\)\]",
        rust,
        re.S,
    )
    if match is None:
        fail("missing recursive Kagemusha redeem Execute implementation")
    body = match.group("body")

    for needle, label in (
        ("Mint::asset_numeric(", "mint construction"),
        ("mint.execute(authority, state_transaction)", "mint execution"),
    ):
        count = body.count(needle)
        if count != 1:
            fail(
                "recursive Kagemusha redeem must have exactly one production "
                f"{label} after all lineage, proof, and nullifier gates; found {count}"
            )

    ordered_needles = (
        "self.bundle\n                .validate_public_input_binding()",
        "let redeem_nullifiers =",
        "resolve_kagemusha_unshield_verifier(",
        "ensure_kagemusha_recursive_redeem_public_inputs(",
        "let block_height = state_transaction.block_height();",
        "crate::zk::preverify_kagemusha_recursive_spend_bundle_with_record_at_height(",
        "register_confidential_proof(self.bundle.recursive_proof.proof.bytes.len())",
        "ensure_kagemusha_recursive_lineage_verifier_records_registered(",
        "crate::zk::verify_kagemusha_recursive_spend_lineage_witness_with_record_resolver_at_height(",
        "crate::zk::ensure_kagemusha_recursive_spend_chain_admission_proves_lineage(",
        "crate::zk::verify_kagemusha_recursive_spend_bundle_with_record_at_height(",
        "state_transaction.register_nullifiers(redeem_nullifiers.len())",
        "state_transaction.register_confidential_proof(self.redeem_proof.proof.bytes.len())",
        "crate::zk::verify_backend_with_timing_checked(",
        "if !report.ok",
        "st.nullifiers.insert(nullifier)",
        "state_transaction.world.zk_assets.insert(def_id.clone(), st)",
        "Mint::asset_numeric(",
        "mint.execute(authority, state_transaction)",
    )
    cursor = 0
    for needle in ordered_needles:
        index = body.find(needle, cursor)
        if index < 0:
            fail(
                "recursive Kagemusha redeem execution order no longer gates mint "
                f"behind lineage, proof, and nullifier checks: missing {needle}"
            )
        cursor = index + len(needle)


def check_rust_reserved_lineage_policy():
    rust = read("crates/iroha_data_model/src/offline/mod.rs")
    max_hops = re.search(
        r"KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1\s*:\s*u32\s*=\s*(\d+)\s*;",
        rust,
    )
    if max_hops is None:
        fail("missing Rust witnessless Reserved-lineage max-hop constant")
    if int(max_hops.group(1)) != 64:
        fail(
            "witnessless Reserved-lineage max-hop constant must be 64 for "
            "production accumulator transition admission"
        )
    if not re.search(
        r"KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1\s*:\s*bool\s*=\s*true\s*;",
        rust,
    ):
        fail(
            "missing Rust transition-circuit-wired true guard for witnessless Reserved-lineage"
        )
    if "KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_DOMAIN" not in rust:
        fail("missing Rust transition-profile binding digest domain")
    if "transition_profile_binding_digest_limb0" not in rust:
        fail("missing Rust recursive proof public-input schema transition binding limbs")
    if "append_opening_preflight_digest_limb0" not in rust:
        fail("missing Rust recursive proof public-input schema append opening preflight limbs")
    if "append_boundary_digest_limb0" not in rust:
        fail("missing Rust recursive proof public-input schema append boundary limbs")
    if "pub transition_profile_binding_digest: [u8; 32]" not in rust:
        fail("missing Rust recursive spend transition binding digest fields")
    if "pub append_opening_preflight_digest: [u8; 32]" not in rust:
        fail("missing Rust recursive spend append opening preflight digest fields")
    if "pub append_boundary_digest: [u8; 32]" not in rust:
        fail("missing Rust recursive proof/accumulator append boundary digest field")
    if (
        "accumulator.append_boundary_digest = [0u8; Hash::LENGTH]" not in rust
        or "append_boundary_digest: accumulator.append_boundary_digest" not in rust
    ):
        fail("missing Rust non-circular accumulator/public-input append boundary binding")

    for name, needles in (
        (
            "can_redeem_kagemusha_recursive_spend_witnessless",
            (
                "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1",
                "is_kagemusha_recursive_spend_lineage_proof_circuit_id(proof_circuit_id)",
                "hop_count >= 1",
                "hop_count <= KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1",
            ),
        ),
        (
            "can_append_kagemusha_recursive_spend_lineage_witnessless",
            (
                "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1",
                "previous_hop_count >= 1",
                "previous_hop_count < KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1",
            ),
        ),
    ):
        match = re.search(
            rf"pub\s+fn\s+{name}\s*\([^)]*\)\s*->\s*bool\s*\{{(?P<body>.*?)\n\}}",
            rust,
            re.S,
        )
        if match is None:
            fail(f"missing Rust helper {name}")
        body = re.sub(r"\s+", " ", match.group("body"))
        for needle in needles:
            if needle not in body:
                fail(f"Rust helper {name} is missing Reserved-lineage policy guard: {needle}")

    core = read("crates/iroha_core/src/zk.rs")
    if not re.search(
        r"KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_INSTANCE_COLUMNS\s*:\s*usize\s*=\s*59\s*;",
        core,
    ):
        fail("recursive Kagemusha proof public instance width must remain 59")
    if "KAGEMUSHA_RECURSIVE_AGGREGATION_TRANSITION_PROFILE_BINDING_START_INDEX" not in core:
        fail("missing recursive Kagemusha transition binding public-instance index")
    if "KAGEMUSHA_RECURSIVE_AGGREGATION_APPEND_OPENING_PREFLIGHT_START_INDEX" not in core:
        fail("missing recursive Kagemusha append opening preflight public-instance index")
    if "KAGEMUSHA_RECURSIVE_AGGREGATION_APPEND_BOUNDARY_START_INDEX" not in core:
        fail("missing recursive Kagemusha append boundary public-instance index")
    data_model = read("crates/iroha_data_model/src/offline/mod.rs")
    for needle in (
        "fn validate_kagemusha_recursive_spend_append_output_selection",
        "is_supported_kagemusha_recursive_spend_append_proof_transition",
        "is_kagemusha_recursive_spend_lineage_append_output_circuit_id",
        "normalize_kagemusha_recursive_spend_append_output_proof_circuit_id",
        "can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id",
        "reserved previous builder accepts structurally valid reserved output",
    ):
        if needle not in data_model:
            fail(
                "Reserved-lineage request validation must separate structural "
                f"transition support from current proving capability: {needle}"
            )
    for needle in (
        "one-hop recursive verifier slice requires verifier witness count 1",
        "one-hop recursive verifier slice requires hop count 1",
        "witness_count - Expression::Constant(Scalar::from(1))",
        "hop_count - Expression::Constant(Scalar::from(1))",
        "KagemushaRecursiveAggregationOneHopVerifierSlice",
        "kagemusha_recursive_spend_lineage_one_hop_backend_opening_len",
        "preflight_kagemusha_recursive_spend_lineage_append_accumulator_opening_contract",
        "Kagemusha Reserved-lineage append accumulator opening preflight digest mismatch",
        "ensure_kagemusha_recursive_spend_lineage_witnessless_append_available",
        "KagemushaRecursiveAggregationAppendVerifierSlice",
        "kagemusha_recursive_spend_lineage_append_vk_box_from_pallas_open_envelope_archive",
        "kagemusha_recursive_spend_lineage_backend_profile",
    ):
        if needle not in core:
            fail(
                "witnessless Reserved-lineage admission is missing verifier-slice "
                f"coverage: {needle}"
            )


def check_docs_reserved_lineage_policy():
    forbidden = [
        re.compile(
            r"witnessless\s+Reserved-lineage\s+redeem\s+requests\s+are\s+emitted\s+only\s+inside\s+the\s+one-hop",
            re.I,
        ),
        re.compile(
            r"metadata-valid\s+one-hop\s+Reserved-lineage\s+requests\s+can\s+serialize\s+witnessless\s+redeem",
            re.I,
        ),
        re.compile(
            r"chain-admission\s+checks\.\s+Those\s+checks\s+admit\s+only\s+the\s+one-hop\s+verifier-slice",
            re.I,
        ),
        re.compile(r"WITNESSLESS_MAX_HOPS_V1[^.\n]*0", re.I),
        re.compile(r"transition[^.\n]*wired[^.\n]*false", re.I),
        re.compile(r"witnessless\s+Reserved-lineage\s+append[^.\n]*disabled", re.I),
    ]
    required = re.compile(
        r"(WITNESSLESS_MAX_HOPS_V1[^.\n]*64|64-hop|64\s+hops|"
        r"witnessless[^.\n]*Reserved-lineage[^.\n]*(enabled|available|admitted))",
        re.I,
    )
    for relative in DOC_PATHS:
        text = read(relative)
        if required.search(text) is None:
            fail(f"{relative} does not document the enabled witnessless Reserved-lineage boundary")
        for pattern in forbidden:
            if pattern.search(text):
                fail(f"{relative} contains stale disabled witnessless Reserved-lineage claim")

    status_text = read("status.md")
    if re.search(
        r"witnessless\s+chain\s+redemption\s+is\s+admitted\s+only\s+inside\s+the\s+wired\s+one-hop\s+verifier-slice\s+bound",
        status_text,
        re.I,
    ):
        fail("status.md contains stale one-hop witnessless chain-redemption boundary")

    offline_doc = re.sub(r"\s+", " ", read("docs/source/offline_kagemusha.md"))
    if "All eight entry points accept" in offline_doc:
        fail("docs/source/offline_kagemusha.md contains stale ABI-6 eight-entry wording")
    for needle in (
        "Bridge ABI 6 introduced, and ABI 6-or-later bridges expose, the production recursive spendable-cash entry points:",
        "`connect_norito_kagemusha_recursive_spend_lineage_append_boundary`",
        "All nine entry points accept and return raw Norito archives",
        "SDKs must treat `previous_recursive_proof_open_envelopes_archive` as opaque native prover material",
        "must not construct, rewrite, or mutate it",
        "the native bridge and SDK append wrappers validate the metadata tuple",
        "(`vk_commitment`, `public_inputs_schema_hash`, `domain_tag`)",
        "against the exact previous bundle before proving or returning output bytes",
        "native bridge and SDK `verify` results are split between offline spendability and chain admission",
        "native bridge and SDK `redeem` entry points are fail-closed on the chain-admission gate",
        "Native bridge and SDK verifier wrappers reject malformed Reserved-lineage verify request archives before returning a diagnostic result",
        "native availability probes: init, append, both transition-profile helpers, the append-boundary helper, both lineage-witness helpers, verify, and redeem must be callable",
        "Request-backed append transition profiles additionally bind `previous_recursive_proof_open_envelopes_archive_digest`",
        "Reserved-lineage append proof output also binds `append_opening_preflight_digest`",
        "`KagemushaRecursiveSpendLineageAppendOpeningPreflightV1` Norito contract",
        "contract must hash back to the digest and match the previous accumulator digest",
        "SDKs should treat the contract as opaque native verifier metadata",
        "legacy evidence-only helpers omit it for compatibility",
        "The archive-aware Rust helper validates non-empty previous-proof opening archive metadata against the exact previous bundle before hashing it",
        "append transition profiles are built through the archive-aware helper with metadata-bound previous-proof opening archives",
        'In this context, "one-hop Reserved-lineage" means',
        "it is not the multi-hop append verifier",
        "Chain admission validates the Reserved-lineage envelope/profile shape before backend proof verification and accepts two strict public-instance layouts",
        "one-hop init (`witness_count = 1`, `hop_count = 1`, one verifier-slice scalar-projection column)",
        "append (`witness_count = hop_count`, `hop_count > 1`, non-zero transition-profile, append-opening-preflight, append-boundary, and append scalar-projection limb groups)",
        "Any lineage bundle outside `KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1 = 64` is rejected before nullifiers or public assets are touched",
        "Product witnessless append output is reachable below the 64-hop cap",
        "bundles whose hop count is inside `KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1 = 64` can redeem witnesslessly",
        "fixed-proof recursive spend bundle at 1,751 bytes",
        "2,048-byte material-growth ceiling",
        "semantic append transition profile is 2,094 bytes",
        "Reserved-lineage D2D bundle is 3,847 bytes",
        "append Reserved-lineage transition profile is 2,817 bytes",
        "`kagemusha_reserved_lineage_transition_profile_bytes`",
    ):
        if needle not in offline_doc:
            fail(
                "docs/source/offline_kagemusha.md is missing previous-proof opening "
                f"SDK-host boundary documentation: {needle}"
            )

    roadmap_doc = re.sub(r"\s+", " ", read("roadmap.md"))
    for needle in (
        "Bridge ABI 6 adds recursive spend `init`, `append`, both transition-profile helpers, append-boundary derivation, both lineage-witness assembly helpers, `verify`, and `redeem` entry points",
        "complete ABI-6 native surface - init, append, both transition-profile helpers, append-boundary derivation, both lineage-witness helpers, verify, and redeem",
        "without the witness path and append-boundary surface needed for safe redemption",
    ):
        if needle not in roadmap_doc:
            fail(f"roadmap.md is missing complete recursive spend ABI-6 surface documentation: {needle}")


def require_needles(coverage, missing_message):
    for relative, needles in coverage.items():
        text = read(relative)
        for needle in needles:
            if needle not in text:
                fail(f"{relative} {missing_message}: {needle}")


def require_normalized_needles(coverage, missing_message):
    for relative, needles in coverage.items():
        text = re.sub(r"\s+", " ", read(relative))
        for needle in needles:
            normalized_needle = re.sub(r"\s+", " ", needle)
            if normalized_needle not in text:
                fail(f"{relative} {missing_message}: {needle}")


def shared_fixture_manifest():
    try:
        return json.loads(read(SHARED_FIXTURE_PATH))
    except json.JSONDecodeError as error:
        fail(f"{SHARED_FIXTURE_PATH} is not valid JSON: {error}")


def shared_archive_fixture_manifest():
    try:
        return json.loads(read(SHARED_ARCHIVE_FIXTURE_PATH))
    except json.JSONDecodeError as error:
        fail(f"{SHARED_ARCHIVE_FIXTURE_PATH} is not valid JSON: {error}")


def check_shared_fixture_manifest():
    manifest = shared_fixture_manifest()
    expected_symbols = {
        "connect_norito_kagemusha_recursive_spend_init",
        "connect_norito_kagemusha_recursive_spend_append",
        "connect_norito_kagemusha_recursive_spend_transition_profile_init",
        "connect_norito_kagemusha_recursive_spend_transition_profile_append",
        "connect_norito_kagemusha_recursive_spend_lineage_append_boundary",
        "connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result",
        "connect_norito_kagemusha_recursive_spend_lineage_witness_append_result",
        "connect_norito_kagemusha_recursive_spend_verify",
        "connect_norito_kagemusha_recursive_spend_redeem",
    }
    if manifest.get("schema") != "iroha.kagemusha.recursive_spend.abi6.fixture_manifest.v1":
        fail(f"{SHARED_FIXTURE_PATH} has the wrong schema")
    if manifest.get("bridge_abi_version") != 6:
        fail(f"{SHARED_FIXTURE_PATH} must pin bridge ABI 6")
    archive_fixture = manifest.get("archive_fixture", {})
    if archive_fixture.get("path") != SHARED_ARCHIVE_FIXTURE_PATH:
        fail(f"{SHARED_FIXTURE_PATH} must link the shared ABI-6 archive fixture")
    if (
        archive_fixture.get("schema")
        != "iroha.kagemusha.recursive_spend.abi6.archive_fixtures.v1"
    ):
        fail(f"{SHARED_FIXTURE_PATH} must pin the shared ABI-6 archive schema")
    operations = manifest.get("operations")
    if not isinstance(operations, list):
        fail(f"{SHARED_FIXTURE_PATH} must contain an operations list")
    if manifest.get("operation_count") != len(operations) or len(operations) != 9:
        fail(f"{SHARED_FIXTURE_PATH} must contain exactly nine ABI-6 operations")
    symbols = {operation.get("symbol") for operation in operations if isinstance(operation, dict)}
    if symbols != expected_symbols:
        fail(f"{SHARED_FIXTURE_PATH} ABI-6 operation symbols drifted")
    append_witness = next(
        (
            operation
            for operation in operations
            if isinstance(operation, dict)
            and operation.get("name") == "lineage_witness_append_result"
        ),
        None,
    )
    if append_witness is None:
        fail(f"{SHARED_FIXTURE_PATH} is missing the append lineage witness operation")
    if append_witness.get("input_archives") != [
        "KagemushaRecursiveSpendLineageWitnessV1",
        "KagemushaRecursiveSpendAppendRequestV1",
        "KagemushaRecursiveSpendBundleV1",
    ]:
        fail(f"{SHARED_FIXTURE_PATH} append lineage witness inputs drifted")

    proof_circuit_ids = manifest.get("proof_circuit_ids", {})
    for key, expected in (
        ("recursive_aggregation", "kagemusha-recursive-aggregation-v1"),
        ("reserved_lineage", "kagemusha-recursive-spend-lineage-v1"),
        ("reserved_lineage_one_hop", "kagemusha-recursive-spend-lineage-onehop-v1"),
        ("reserved_lineage_append", "kagemusha-recursive-spend-lineage-append-v1"),
    ):
        if proof_circuit_ids.get(key) != expected:
            fail(f"{SHARED_FIXTURE_PATH} circuit id {key} drifted")

    limits = manifest.get("limits", {})
    expected_limits = {
        "compact_token_max_hops": 64,
        "reserved_lineage_witnessless_max_hops": 64,
        "previous_proof_open_envelopes_required_count": 1,
        "previous_proof_open_envelopes_max_bytes": 8 * 1024 * 1024,
        "pallas_open_envelope_max_transcript_label_bytes": 128,
        "native_archive_max_bytes": 64 * 1024 * 1024,
    }
    for key, expected in expected_limits.items():
        if limits.get(key) != expected:
            fail(f"{SHARED_FIXTURE_PATH} limit {key} drifted")

    domains = manifest.get("domains", {})
    for key, expected in (
        ("transition_profile", "iroha:kagemusha:v1:recursive-spend-transition-profile"),
        (
            "lineage_append_boundary_final_note_binding",
            "iroha:kagemusha:recursive-spend-lineage-append-boundary-final-note:v1",
        ),
    ):
        if domains.get(key) != expected:
            fail(f"{SHARED_FIXTURE_PATH} domain {key} drifted")

    benchmarks = manifest.get("payload_benchmarks", {})
    for key, expected in (
        ("semantic_payload_bytes", 1751),
        ("semantic_payload_max_bytes", 2048),
        ("semantic_transition_profile_bytes", 2094),
        ("semantic_transition_profile_max_bytes", 3072),
        ("reserved_lineage_payload_bytes", 3847),
        ("reserved_lineage_payload_max_bytes", 8192),
        ("reserved_lineage_transition_profile_bytes", 2817),
        ("reserved_lineage_transition_profile_max_bytes", 4096),
    ):
        if benchmarks.get(key) != expected:
            fail(f"{SHARED_FIXTURE_PATH} payload benchmark {key} drifted")

    if benchmarks.get("hops") != [1, 2, 3, 5, 8, 13, 21, 34, 55, 64]:
        fail(f"{SHARED_FIXTURE_PATH} benchmark hop series drifted")

    require_normalized_needles(
        SHARED_FIXTURE_COVERAGE,
        "is missing shared recursive spend ABI-6 fixture coverage",
    )


def check_shared_archive_fixture_manifest():
    manifest = shared_archive_fixture_manifest()
    if manifest.get("schema") != "iroha.kagemusha.recursive_spend.abi6.archive_fixtures.v1":
        fail(f"{SHARED_ARCHIVE_FIXTURE_PATH} has the wrong schema")
    if manifest.get("bridge_abi_version") != 6:
        fail(f"{SHARED_ARCHIVE_FIXTURE_PATH} must pin bridge ABI 6")
    archives = manifest.get("archives")
    if not isinstance(archives, list):
        fail(f"{SHARED_ARCHIVE_FIXTURE_PATH} must contain an archives list")
    expected = {
        "init_request": ("init", "KagemushaRecursiveSpendInitRequestV1"),
        "init_bundle": ("init", "KagemushaRecursiveSpendBundleV1"),
        "transition_profile_init": (
            "transition_profile_init",
            "KagemushaRecursiveSpendTransitionProfileV1",
        ),
        "append_request": ("append", "KagemushaRecursiveSpendAppendRequestV1"),
        "append_bundle": ("append", "KagemushaRecursiveSpendBundleV1"),
        "transition_profile_append": (
            "transition_profile_append",
            "KagemushaRecursiveSpendTransitionProfileV1",
        ),
        "lineage_append_boundary": (
            "lineage_append_boundary",
            "KagemushaRecursiveSpendLineageAppendBoundaryV1",
        ),
        "lineage_witness_from_init_result": (
            "lineage_witness_from_init_result",
            "KagemushaRecursiveSpendLineageWitnessV1",
        ),
        "lineage_witness_append_result": (
            "lineage_witness_append_result",
            "KagemushaRecursiveSpendLineageWitnessV1",
        ),
        "verify_request": ("verify", "KagemushaRecursiveSpendVerifyRequestV1"),
        "verify_result": ("verify", "KagemushaRecursiveSpendVerifyResultV1"),
        "redeem_request": ("redeem", "KagemushaRecursiveSpendRedeemRequestV1"),
        "redeem_instruction": ("redeem", "RedeemKagemushaRecursive"),
    }
    by_name = {
        archive.get("name"): archive
        for archive in archives
        if isinstance(archive, dict)
    }
    if set(by_name) != set(expected):
        fail(f"{SHARED_ARCHIVE_FIXTURE_PATH} archive names drifted")
    for name, (operation, norito_type) in expected.items():
        archive = by_name[name]
        if archive.get("operation") != operation:
            fail(f"{SHARED_ARCHIVE_FIXTURE_PATH} operation for {name} drifted")
        if archive.get("norito_type") != norito_type:
            fail(f"{SHARED_ARCHIVE_FIXTURE_PATH} Norito type for {name} drifted")
        payload_b64 = archive.get("bytes_base64")
        if not isinstance(payload_b64, str) or not payload_b64:
            fail(f"{SHARED_ARCHIVE_FIXTURE_PATH} {name} is missing bytes_base64")
        try:
            payload = base64.b64decode(payload_b64, validate=True)
        except ValueError as error:
            fail(f"{SHARED_ARCHIVE_FIXTURE_PATH} {name} has invalid base64: {error}")
        if archive.get("byte_len") != len(payload):
            fail(f"{SHARED_ARCHIVE_FIXTURE_PATH} byte_len for {name} does not match bytes")
        sha256_hex = archive.get("sha256_hex")
        if sha256_hex != hashlib.sha256(payload).hexdigest():
            fail(f"{SHARED_ARCHIVE_FIXTURE_PATH} sha256 for {name} does not match bytes")


def check_adversarial_coverage():
    require_needles(
        ADVERSARIAL_COVERAGE,
        "is missing Reserved-lineage adversarial coverage",
    )


def check_sdk_helper_edge_coverage():
    require_needles(
        SDK_HELPER_EDGE_COVERAGE,
        "is missing SDK Reserved-lineage helper edge coverage",
    )


def check_sdk_append_cap_binding_coverage():
    require_normalized_needles(
        SDK_APPEND_CAP_BINDING_COVERAGE,
        "is missing SDK Reserved-lineage append cap constant binding",
    )


def check_native_output_cap_coverage():
    require_normalized_needles(
        NATIVE_OUTPUT_CAP_COVERAGE,
        "is missing native Kagemusha output cap coverage",
    )


def check_reserved_lineage_profile_split_coverage():
    require_needles(
        RESERVED_LINEAGE_PROFILE_SPLIT_COVERAGE,
        "is missing Reserved-lineage one-hop/append profile split coverage",
    )


def check_verify_result_fail_closed_coverage():
    require_needles(
        VERIFY_RESULT_FAIL_CLOSED_COVERAGE,
        "is missing verify-result fail-closed flag coverage",
    )


def check_payload_benchmark_source_coverage():
    require_needles(
        PAYLOAD_BENCH_SOURCE_COVERAGE,
        "is missing archive-aware payload benchmark coverage",
    )


def run_checks():
    check_workflow_paths_cover_policy_sources()
    check_workflow_preserves_in_progress_runs()
    check_workflow_runs_main_guards()
    check_workflow_runs_header_negative_controls()
    check_workflow_runs_python_sdk_tests()
    check_workflow_runs_payload_reducer_controls()
    check_workflow_runs_policy_negative_controls()
    check_core_redeem_execution_order()
    check_rust_reserved_lineage_policy()
    check_docs_reserved_lineage_policy()
    check_shared_fixture_manifest()
    check_shared_archive_fixture_manifest()
    check_adversarial_coverage()
    check_sdk_helper_edge_coverage()
    check_sdk_append_cap_binding_coverage()
    check_native_output_cap_coverage()
    check_reserved_lineage_profile_split_coverage()
    check_verify_result_fail_closed_coverage()
    check_payload_benchmark_source_coverage()


if mode == "--negative-control":
    target = "javascript/iroha_js/test/kagemushaRecursiveSpend.test.js"
    source = read(target)
    mutated = source.replace("Number.NaN", "Number.NotANumber")
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate SDK edge-case coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected Reserved-lineage policy drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Reserved-lineage policy drift was not detected")

if mode == "--negative-control-sdk-selector-edge":
    target = "javascript/iroha_js/test/kagemushaRecursiveSpend.test.js"
    source = read(target)
    mutated = source.replace(
        "semantic previous proofs cannot select Reserved-lineage output",
        "semantic previous proofs selector edge case",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate SDK selector edge coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected SDK Reserved-lineage selector edge drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: SDK selector edge drift was not detected")

if mode == "--negative-control-sdk-preferred-cap-edge":
    target = "javascript/iroha_js/test/kagemushaRecursiveSpend.test.js"
    source = read(target)
    mutated = source.replace(
        "preferred append selector falls back at the witnessless hop cap",
        "preferred append selector cap fallback",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate SDK preferred cap edge coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected SDK Reserved-lineage preferred cap edge drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: SDK preferred cap edge drift was not detected")

if mode == "--negative-control-js-package-dist-selector-edge":
    target = "javascript/iroha_js/test/package_dist.test.js"
    source = read(target)
    mutated = source.replace(
        "semantic previous proofs cannot select Reserved-lineage output",
        "semantic previous proofs selector edge case",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate JavaScript package-dist selector edge coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected JavaScript package-dist selector edge drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript package-dist selector edge drift was not detected")

if mode == "--negative-control-python-hop-edges":
    target = "python/iroha_python/tests/kagemusha_test.py"
    source = read(target)
    mutated = source.replace('float("nan")', 'float("not-a-number")')
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate Python hop-count edge coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected Python Reserved-lineage hop-count policy drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python hop-count policy drift was not detected")

if mode == "--negative-control-js-hop-edges":
    target = "javascript/iroha_js/test/kagemushaRecursiveSpend.test.js"
    source = read(target)
    mutated = source.replace(
        "canAppendKagemushaRecursiveSpendWitnesslessLineage(1n)",
        "canAppendKagemushaRecursiveSpendWitnesslessLineage(BigInt(1))",
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate JavaScript BigInt hop-count coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected JavaScript Reserved-lineage BigInt hop-count policy drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript BigInt hop-count policy drift was not detected")

if mode == "--negative-control-js-package-dist-hop-edges":
    target = "javascript/iroha_js/test/package_dist.test.js"
    source = read(target)
    mutated = source.replace(
        "canAppendKagemushaRecursiveSpendWitnesslessLineage(1n)",
        "canAppendKagemushaRecursiveSpendWitnesslessLineage(BigInt(1))",
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate JavaScript package-dist BigInt hop-count coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected JavaScript package-dist Reserved-lineage BigInt hop-count policy drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript package-dist BigInt hop-count policy drift was not detected")

if mode == "--negative-control-sdk-append-cap-binding":
    target = "javascript/iroha_js/src/crypto.js"
    source = read(target)
    mutated = source.replace(
        "return previousHopCount < KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS;",
        "return previousHopCount < 64;",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate SDK append cap binding")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected SDK Reserved-lineage append cap binding drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: SDK append cap binding drift was not detected")

if mode == "--negative-control-native-output-cap":
    target = "javascript/iroha_js/src/crypto.js"
    source = read(target)
    mutated = source.replace(
        "export const KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES = 64 * 1024 * 1024;",
        "export const KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES = 64 * 1024;",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate native output cap coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected native Kagemusha output cap drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: native output cap drift was not detected")

if mode == "--negative-control-shared-fixture-manifest":
    target = SHARED_FIXTURE_PATH
    source = read(target)
    mutated = source.replace('"operation_count": 9', '"operation_count": 8', 1)
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate shared fixture manifest")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected shared recursive spend ABI-6 fixture drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: shared fixture manifest drift was not detected")

if mode == "--negative-control-shared-archive-fixture":
    target = SHARED_ARCHIVE_FIXTURE_PATH
    source = read(target)
    mutated = source.replace(
        '"sha256_hex": "c5402b3ea6aeb35ce12607344304b858273f8589e2b3887708a86cb19665ce68"',
        '"sha256_hex": "00402b3ea6aeb35ce12607344304b858273f8589e2b3887708a86cb19665ce68"',
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate shared archive fixture")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected shared recursive spend ABI-6 archive drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: shared archive fixture drift was not detected")

if mode == "--negative-control-data-model-append-cap-boundary":
    target = "crates/iroha_data_model/src/offline/mod.rs"
    source = read(target)
    mutated = source.replace(
        "Reserved-lineage append request at the witnessless hop cap must reject before proving",
        "Reserved-lineage append request at the hop edge",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate data-model append cap boundary coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected data-model append cap boundary drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: data-model append cap boundary drift was not detected")

if mode == "--negative-control-data-model-self-consistent-boundary":
    target = "crates/iroha_data_model/src/offline/mod.rs"
    source = read(target)
    mutated = source.replace(
        "assert_self_consistent_forged_boundary_rejected",
        "assert_profile_bound_forged_boundary_rejected",
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate self-consistent append-boundary coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected self-consistent append-boundary drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: self-consistent append-boundary drift was not detected")

if mode == "--negative-control-data-model-transition-profile-current-hop-sets":
    target = "crates/iroha_data_model/src/offline/mod.rs"
    source = read(target)
    mutated = source.replace(
        "validate_kagemusha_unique_input_output_sets",
        "validate_kagemusha_canonical_input_output_sets",
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate transition-profile current-hop set coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected transition-profile current-hop set drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: transition-profile current-hop set drift was not detected")

if mode == "--negative-control-core-append-cap-boundary":
    target = "crates/iroha_core/src/zk.rs"
    source = read(target)
    mutated = source.replace(
        "direct Reserved-lineage append at the witnessless hop cap must reject before input parsing",
        "direct Reserved-lineage append at the hop edge",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate core append cap boundary coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected core append cap boundary drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: core append cap boundary drift was not detected")

if mode == "--negative-control-core-lineage-profile-split":
    target = "crates/iroha_core/src/zk.rs"
    source = read(target)
    mutated = source.replace(
        "Reserved-lineage one-hop and append verifier records must coexist under distinct circuit ids",
        "Reserved-lineage verifier records must coexist",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate Reserved-lineage profile split coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected Reserved-lineage profile split drift")
        print(str(error).splitlines()[0])
    raise SystemExit(0)
    raise SystemExit("negative control failed: Reserved-lineage profile split drift was not detected")

if mode == "--negative-control-core-proof-chain-accumulator":
    target = "crates/iroha_data_model/src/offline/mod.rs"
    source = read(target)
    mutated = source.replace(
        "proof-byte splice is bound into accumulator state",
        "proof-byte splice may be detached from accumulator state",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate proof-chain accumulator coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected proof-chain accumulator drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: proof-chain accumulator drift was not detected")

if mode == "--negative-control-core-vesta-ipa-h-fold":
    target = "crates/iroha_core/src/zk.rs"
    source = read(target)
    mutated = source.replace(
        "kagemusha_non_native_vesta_ipa_verifier_shared_table_batch_preflight_rejects_h_generator_fold_splice",
        "kagemusha_non_native_vesta_ipa_verifier_shared_table_batch_preflight_allows_h_generator_fold_splice",
        1,
    )
    mutated = mutated.replace(
        "accumulator H fold mismatch",
        "accumulator fold mismatch",
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate non-native Vesta IPA H-fold coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected non-native Vesta IPA H-fold drift")
        print(str(error).splitlines()[0])
    raise SystemExit(0)
    raise SystemExit("negative control failed: non-native Vesta IPA H-fold drift was not detected")

if mode == "--negative-control-core-vesta-ipa-g-fold":
    target = "crates/iroha_core/src/zk.rs"
    source = read(target)
    mutated = source.replace(
        "kagemusha_non_native_vesta_ipa_verifier_from_pallas_witness_rejects_generator_fold_splice",
        "kagemusha_non_native_vesta_ipa_verifier_from_pallas_witness_allows_generator_fold_splice",
        1,
    )
    mutated = mutated.replace(
        "accumulator G fold mismatch",
        "accumulator fold mismatch",
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate non-native Vesta IPA G-fold coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected non-native Vesta IPA G-fold drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: non-native Vesta IPA G-fold drift was not detected")

if mode == "--negative-control-data-model-lineage-key-package-binding":
    target = "crates/iroha_data_model/src/offline/mod.rs"
    source = read(target)
    mutated = source.replace(
        "kagemusha_lineage_key_artifact_packages_reject_profile_splices",
        "kagemusha_lineage_key_artifact_packages_allow_profile_splices",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate data-model lineage key package-binding coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected data-model lineage key package-binding drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: data-model lineage key package-binding drift was not detected")

if mode == "--negative-control-core-opening-preflight-splices":
    target = "crates/iroha_core/src/zk.rs"
    source = read(target)
    mutated = source.replace(
        "forged contract current-hop proof hash must reject",
        "forged contract current-hop proof hash must be checked",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate append opening-preflight splice coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected append opening-preflight splice drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: append opening-preflight splice drift was not detected")

if mode == "--negative-control-core-current-hop-opening-metadata-splice":
    target = "crates/iroha_core/src/zk.rs"
    source = read(target)
    mutated = source.replace(
        "metadata-spliced current-hop opening archive must reject",
        "metadata-spliced current-hop opening archive may pass",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate current-hop opening metadata splice coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected current-hop opening metadata splice drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: current-hop opening metadata splice drift was not detected")

if mode == "--negative-control-core-append-verifier-slice-preflight-binding":
    target = "crates/iroha_core/src/zk.rs"
    source = read(target)
    mutated = source.replace(
        "append slice must reject detached current-hop preflight",
        "append slice may accept detached current-hop preflight",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate append verifier-slice preflight binding coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected append verifier-slice preflight binding drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: append verifier-slice preflight binding drift was not detected")

if mode == "--negative-control-core-one-hop-verifier-slice-evidence-binding":
    target = "crates/iroha_core/src/zk.rs"
    source = read(target)
    mutated = source.replace(
        "one-hop verifier-slice evidence binding must reject params fingerprint splice",
        "one-hop verifier-slice evidence binding may accept params fingerprint splice",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate one-hop verifier-slice evidence binding coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected one-hop verifier-slice evidence binding drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: one-hop verifier-slice evidence binding drift was not detected")

if mode == "--negative-control-core-fold-overlap-predecode":
    target = "crates/iroha_core/src/zk.rs"
    source = read(target)
    mutated = source.replace(
        "record-backed cross-hop overlap error should come before proof decoding",
        "record-backed cross-hop overlap may decode proof first",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate checked-fold overlap predecode coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected checked-fold overlap predecode drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: checked-fold overlap predecode drift was not detected")

if mode == "--negative-control-core-lineage-witness-fold-predecode":
    target = "crates/iroha_core/src/zk.rs"
    source = read(target)
    mutated = source.replace(
        "lineage witness root-continuity error should come before Pallas archive decoding",
        "lineage witness root-continuity error may decode Pallas first",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate lineage witness fold predecode coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected lineage witness fold predecode drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: lineage witness fold predecode drift was not detected")

if mode == "--negative-control-core-lineage-witness-record-predecode":
    target = "crates/iroha_core/src/zk.rs"
    source = read(target)
    mutated = source.replace(
        "lineage witness verifier-record error should come before Pallas archive decoding",
        "lineage witness verifier-record error may decode Pallas first",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate lineage witness record predecode coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected lineage witness verifier-record predecode drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: lineage witness verifier-record predecode drift was not detected")

if mode == "--negative-control-core-lineage-witness-note-predecode":
    target = "crates/iroha_core/src/zk.rs"
    source = read(target)
    mutated = source.replace(
        "lineage witness current-note error should come before Pallas archive decoding",
        "lineage witness current-note error may decode Pallas first",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate lineage witness current-note predecode coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected lineage witness current-note predecode drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: lineage witness current-note predecode drift was not detected")

if mode == "--negative-control-core-lineage-witness-note-binding-predecode":
    target = "crates/iroha_core/src/zk.rs"
    source = read(target)
    mutated = source.replace(
        "lineage witness current-note binding error should come before Pallas archive decoding",
        "lineage witness current-note binding error may decode Pallas first",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate lineage witness current-note binding predecode coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected lineage witness current-note binding predecode drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: lineage witness current-note binding predecode drift was not detected")

if mode == "--negative-control-core-lineage-witness-handoff-predecode":
    target = "crates/iroha_core/src/zk.rs"
    source = read(target)
    mutated = source.replace(
        "lineage witness append-handoff error should come before Pallas archive decoding",
        "lineage witness append-handoff error may decode Pallas first",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate lineage witness append-handoff predecode coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected lineage witness append-handoff predecode drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: lineage witness append-handoff predecode drift was not detected")

if mode == "--negative-control-core-lineage-witness-final-bundle-predecode":
    target = "crates/iroha_core/src/zk.rs"
    source = read(target)
    mutated = source.replace(
        "lineage witness final-bundle error should come before Pallas archive decoding",
        "lineage witness final-bundle error may decode Pallas first",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate lineage witness final-bundle predecode coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected lineage witness final-bundle predecode drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: lineage witness final-bundle predecode drift was not detected")

if mode == "--negative-control-core-recursive-compact-public-instance-shape":
    target = "crates/iroha_core/src/zk.rs"
    source = read(target)
    mutated = source.replace(
        "recursive compact token multi-row public instances must reject",
        "recursive compact token multi-row public instances may pass",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate recursive compact public instance shape coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected recursive compact public instance shape drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: recursive compact public instance shape drift was not detected")

if mode == "--negative-control-core-recursive-compact-pallas-count":
    target = "crates/iroha_core/src/zk.rs"
    source = read(target)
    mutated = source.replace(
        "extra compact Pallas opening must reject before unavailable",
        "extra compact Pallas opening may return unavailable",
        1,
    )
    mutated = mutated.replace(
        "missing compact Pallas opening must reject before unavailable",
        "missing compact Pallas opening may return unavailable",
        1,
    )
    mutated = mutated.replace(
        "duplicated multi-hop compact Pallas archive must reject before unavailable",
        "duplicated multi-hop compact Pallas archive may return unavailable",
        1,
    )
    mutated = mutated.replace(
        "height-aware duplicated multi-hop compact Pallas archive must reject before unavailable",
        "height-aware duplicated multi-hop compact Pallas archive may return unavailable",
        1,
    )
    mutated = mutated.replace(
        "reordered multi-hop compact Pallas archive must reject before unavailable",
        "reordered multi-hop compact Pallas archive may return unavailable",
        1,
    )
    mutated = mutated.replace(
        "height-aware reordered multi-hop compact Pallas archive must reject before unavailable",
        "height-aware reordered multi-hop compact Pallas archive may return unavailable",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate recursive compact Pallas opening count coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected recursive compact Pallas opening count drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: recursive compact Pallas opening count drift was not detected")

if mode == "--negative-control-core-recursive-compact-pallas-metadata":
    target = "crates/iroha_core/src/zk.rs"
    source = read(target)
    mutated = source.replace(
        "forged multi-hop compact Pallas metadata must reject before unavailable",
        "forged multi-hop compact Pallas metadata may return unavailable",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate recursive compact Pallas metadata coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected recursive compact Pallas metadata drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: recursive compact Pallas metadata drift was not detected")

if mode == "--negative-control-core-recursive-compact-cid-spoof-key":
    target = "crates/iroha_core/src/zk.rs"
    source = read(target)
    mutated = source.replace(
        "CID-spoofed ABI-7 compact verifier key must reject before unavailable",
        "CID-spoofed ABI-7 compact verifier key may return unavailable",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate recursive compact CID-spoof key coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected recursive compact CID-spoof key drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: recursive compact CID-spoof key drift was not detected")

if mode == "--negative-control-core-recursive-spend-compact-projection-token":
    target = "crates/iroha_core/src/zk.rs"
    source = read(target)
    mutated = source.replace(
        "one-hop side scalar projection splice must reject",
        "one-hop side scalar projection splice may pass",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate recursive spend compact projection token coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected recursive spend compact projection token drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: recursive spend compact projection token drift was not detected")

if mode == "--negative-control-bridge-recursive-compact-public-instance-shape":
    target = "crates/connect_norito_bridge/src/lib.rs"
    source = read(target)
    mutated = source.replace(
        "ABI-7 compact verifier must reject multi-row public instances before returning a soft invalid result",
        "ABI-7 compact verifier may soft-invalid multi-row public instances",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate bridge recursive compact public instance shape coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected bridge recursive compact public instance shape drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: bridge recursive compact public instance shape drift was not detected")

if mode == "--negative-control-bridge-recursive-compact-pallas-count":
    target = "crates/connect_norito_bridge/src/lib.rs"
    source = read(target)
    mutated = source.replace(
        "ABI-7 compact prover must reject extra valid Pallas opening archives before the unavailable gate",
        "ABI-7 compact prover may accept extra valid Pallas opening archives",
        1,
    )
    mutated = mutated.replace(
        "ABI-7 compact prover must reject missing valid Pallas opening archives before the unavailable gate",
        "ABI-7 compact prover may accept missing valid Pallas opening archives",
        1,
    )
    mutated = mutated.replace(
        "ABI-7 compact prover must reject duplicated multi-hop valid Pallas opening archives before the unavailable gate",
        "ABI-7 compact prover may accept duplicated multi-hop valid Pallas opening archives",
        1,
    )
    mutated = mutated.replace(
        "ABI-7 compact prover must reject reordered valid Pallas opening archives before the unavailable gate",
        "ABI-7 compact prover may accept reordered valid Pallas opening archives",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate bridge recursive compact Pallas opening count coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected bridge recursive compact Pallas opening count drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: bridge recursive compact Pallas opening count drift was not detected")

if mode == "--negative-control-bridge-recursive-compact-pallas-metadata":
    target = "crates/connect_norito_bridge/src/lib.rs"
    source = read(target)
    mutated = source.replace(
        "ABI-7 compact prover must reject forged multi-hop Pallas metadata before the unavailable gate",
        "ABI-7 compact prover may accept forged multi-hop Pallas metadata",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate bridge recursive compact Pallas metadata coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected bridge recursive compact Pallas metadata drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: bridge recursive compact Pallas metadata drift was not detected")

if mode == "--negative-control-bridge-recursive-compact-vk-hash":
    target = "crates/connect_norito_bridge/src/lib.rs"
    source = read(target)
    mutated = source.replace(
        "ABI-7 compact verifier must reject non-canonical envelope verifier-key hashes before returning a soft invalid result",
        "ABI-7 compact verifier may soft-invalid non-canonical envelope verifier-key hashes",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate bridge recursive compact verifier-key hash coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected bridge recursive compact verifier-key hash drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: bridge recursive compact verifier-key hash drift was not detected")

if mode == "--negative-control-js-host-recursive-compact-vk-hash":
    target = "crates/iroha_js_host/src/lib.rs"
    source = read(target)
    mutated = source.replace(
        "recursive compact token with forged verifier-key hash must reject",
        "recursive compact token with forged verifier-key hash may soft-invalid",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate JS host recursive compact verifier-key hash coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected JS host recursive compact verifier-key hash drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JS host recursive compact verifier-key hash drift was not detected")

if mode == "--negative-control-js-host-recursive-compact-pallas-count":
    target = "crates/iroha_js_host/src/lib.rs"
    source = read(target)
    mutated = source.replace(
        "recursive compact prover must reject extra valid Pallas opening archive",
        "recursive compact prover may accept extra valid Pallas opening archive",
        1,
    )
    mutated = mutated.replace(
        "recursive compact prover must reject missing valid Pallas opening archive",
        "recursive compact prover may accept missing valid Pallas opening archive",
        1,
    )
    mutated = mutated.replace(
        "recursive compact prover must reject duplicated multi-hop valid Pallas opening archive",
        "recursive compact prover may accept duplicated multi-hop valid Pallas opening archive",
        1,
    )
    mutated = mutated.replace(
        "recursive compact prover must reject reordered valid Pallas opening archive",
        "recursive compact prover may accept reordered valid Pallas opening archive",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate JS host recursive compact Pallas opening count coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected JS host recursive compact Pallas opening count drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JS host recursive compact Pallas opening count drift was not detected")

if mode == "--negative-control-js-host-recursive-compact-pallas-metadata":
    target = "crates/iroha_js_host/src/lib.rs"
    source = read(target)
    mutated = source.replace(
        "recursive compact prover must reject forged multi-hop Pallas metadata",
        "recursive compact prover may accept forged multi-hop Pallas metadata",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate JS host recursive compact Pallas metadata coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected JS host recursive compact Pallas metadata drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JS host recursive compact Pallas metadata drift was not detected")

if mode == "--negative-control-js-host-recursive-compact-public-instance-shape":
    target = "crates/iroha_js_host/src/lib.rs"
    source = read(target)
    mutated = source.replace(
        "JS host recursive compact verifier must reject multi-row public instances",
        "JS host recursive compact verifier may soft-invalid multi-row public instances",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate JS host recursive compact public instance shape coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected JS host recursive compact public instance shape drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JS host recursive compact public instance shape drift was not detected")

if mode == "--negative-control-js-host-kagemusha-archive-cap":
    target = "crates/iroha_js_host/src/lib.rs"
    source = read(target)
    mutated = source.replace(
        "archive_len > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
        "false && archive_len > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
        1,
    )
    mutated = mutated.replace(
        "KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES + 1",
        "KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate JS host Kagemusha archive cap coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected JS host Kagemusha archive cap drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JS host Kagemusha archive cap drift was not detected")

if mode == "--negative-control-js-host-append-boundary-current-output-set":
    target = "crates/iroha_js_host/src/lib.rs"
    source = read(target)
    mutated = source.replace(
        "JS host append-boundary helper must reject duplicate current-hop outputs",
        "JS host append-boundary helper may accept duplicate current-hop outputs",
        1,
    )
    mutated = mutated.replace(
        "repeats an output commitment",
        "accepts duplicate output commitment",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate JS host append-boundary current-hop output-set coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected JS host append-boundary current-hop output-set drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JS host append-boundary current-hop output-set drift was not detected")

if mode == "--negative-control-python-recursive-compact-vk-hash":
    target = "python/iroha_python/iroha_python_rs/src/lib.rs"
    source = read(target)
    mutated = source.replace(
        "recursive compact token with forged verifier-key hash must reject",
        "recursive compact token with forged verifier-key hash may soft-invalid",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate Python recursive compact verifier-key hash coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected Python recursive compact verifier-key hash drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python recursive compact verifier-key hash drift was not detected")

if mode == "--negative-control-python-recursive-compact-pallas-count":
    target = "python/iroha_python/iroha_python_rs/src/lib.rs"
    source = read(target)
    mutated = source.replace(
        "recursive compact prover must reject extra valid Pallas opening archive",
        "recursive compact prover may accept extra valid Pallas opening archive",
        1,
    )
    mutated = mutated.replace(
        "recursive compact prover must reject missing valid Pallas opening archive",
        "recursive compact prover may accept missing valid Pallas opening archive",
        1,
    )
    mutated = mutated.replace(
        "recursive compact prover must reject duplicated multi-hop valid Pallas opening archive",
        "recursive compact prover may accept duplicated multi-hop valid Pallas opening archive",
        1,
    )
    mutated = mutated.replace(
        "recursive compact prover must reject reordered valid Pallas opening archive",
        "recursive compact prover may accept reordered valid Pallas opening archive",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate Python recursive compact Pallas opening count coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected Python recursive compact Pallas opening count drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python recursive compact Pallas opening count drift was not detected")

if mode == "--negative-control-python-recursive-compact-pallas-metadata":
    target = "python/iroha_python/iroha_python_rs/src/lib.rs"
    source = read(target)
    mutated = source.replace(
        "recursive compact prover must reject forged multi-hop Pallas metadata",
        "recursive compact prover may accept forged multi-hop Pallas metadata",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate Python recursive compact Pallas metadata coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected Python recursive compact Pallas metadata drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python recursive compact Pallas metadata drift was not detected")

if mode == "--negative-control-python-recursive-compact-public-instance-shape":
    target = "python/iroha_python/iroha_python_rs/src/lib.rs"
    source = read(target)
    mutated = source.replace(
        "Python recursive compact verifier must reject multi-row public instances",
        "Python recursive compact verifier may soft-invalid multi-row public instances",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate Python recursive compact public instance shape coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected Python recursive compact public instance shape drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python recursive compact public instance shape drift was not detected")

if mode == "--negative-control-python-kagemusha-archive-cap":
    target = "python/iroha_python/iroha_python_rs/src/lib.rs"
    source = read(target)
    mutated = source.replace(
        "archive_len > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
        "false && archive_len > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
        1,
    )
    mutated = mutated.replace(
        "KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES + 1",
        "KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate Python Kagemusha archive cap coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected Python Kagemusha archive cap drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python Kagemusha archive cap drift was not detected")

if mode == "--negative-control-python-append-boundary-current-output-set":
    target = "python/iroha_python/iroha_python_rs/src/lib.rs"
    source = read(target)
    mutated = source.replace(
        "Python append-boundary helper must reject duplicate current-hop outputs",
        "Python append-boundary helper may accept duplicate current-hop outputs",
        1,
    )
    mutated = mutated.replace(
        "repeats an output commitment",
        "accepts duplicate output commitment",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate Python append-boundary current-hop output-set coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected Python append-boundary current-hop output-set drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python append-boundary current-hop output-set drift was not detected")

if mode == "--negative-control-fixed-window-manifest-digest-splice":
    target = "crates/iroha_core/src/zk.rs"
    source = read(target)
    mutated = source.replace(
        "manifest row splice must change digest",
        "manifest row splice should change digest",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate fixed-window manifest digest splice coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected fixed-window manifest digest splice drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: fixed-window manifest digest splice drift was not detected")

if mode == "--negative-control-workflow":
    target = WORKFLOW_PATH
    source = read(target)
    mutated = source.replace(
        '      - "javascript/iroha_js/test/kagemushaRecursiveSpend.test.js"\n',
        "",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate workflow path coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected fail-closed workflow path drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: workflow path drift was not detected")

if mode == "--negative-control-js-package-dist-workflow":
    target = WORKFLOW_PATH
    source = read(target)
    mutated = source.replace(
        '      - "javascript/iroha_js/test/package_dist.test.js"\n',
        "",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate JS package-dist workflow path coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected JS package-dist workflow path drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JS package-dist workflow path drift was not detected")

if mode == "--negative-control-core-isi-workflow":
    target = WORKFLOW_PATH
    source = read(target)
    mutated = source.replace(
        '      - "crates/iroha_core/src/smartcontracts/isi/offline.rs"\n',
        "",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate core ISI workflow path coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected core ISI workflow path drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: core ISI workflow path drift was not detected")

if mode == "--negative-control-payload-script-workflow":
    target = WORKFLOW_PATH
    source = read(target)
    mutated = source.replace(
        '      - "ci/check_kagemusha_recursive_spend_payload_bench.sh"\n',
        "",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate payload reducer script workflow path")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected payload reducer script workflow path drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: payload reducer script workflow path drift was not detected")

if mode == "--negative-control-ci-guard-script-workflow":
    target = WORKFLOW_PATH
    source = read(target)
    mutated = source.replace(
        '      - "ci/check_connect_norito_bridge_header.sh"\n',
        "",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate CI guard script workflow path")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected CI guard script workflow path drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: CI guard script workflow path drift was not detected")

if mode == "--negative-control-payload-self-test-workflow":
    target = WORKFLOW_PATH
    source = read(target)
    mutated = source.replace(
        "ci/check_kagemusha_recursive_spend_payload_bench.sh --self-test",
        "ci/check_kagemusha_recursive_spend_payload_bench.sh --synthetic-check",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate payload reducer self-test")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected payload reducer self-test workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: payload reducer self-test drift was not detected")

if mode == "--negative-control-payload-self-test-order-workflow":
    target = WORKFLOW_PATH
    source = read(target)
    command = "        run: ci/check_kagemusha_recursive_spend_payload_bench.sh --self-test"
    mutated = source.replace(f"{command}\n", "", 1)
    mutated = mutated.replace(
        "        run: ci/check_kagemusha_recursive_spend_payload_bench.sh\n",
        "        run: ci/check_kagemusha_recursive_spend_payload_bench.sh\n"
        "          ci/check_kagemusha_recursive_spend_payload_bench.sh --self-test\n",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to move payload reducer self-test after benchmark")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected payload reducer self-test ordering drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: payload reducer self-test ordering drift was not detected")

if mode == "--negative-control-payload-missing-payload-workflow":
    target = WORKFLOW_PATH
    source = read(target)
    mutated = source.replace(
        "ci/check_kagemusha_recursive_spend_payload_bench.sh --negative-control-missing-payload",
        "ci/check_kagemusha_recursive_spend_payload_bench.sh --synthetic-missing-payload-check",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate missing-payload reducer command")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected missing-payload reducer workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: missing-payload reducer workflow drift was not detected")

if mode == "--negative-control-payload-negative-controls-workflow":
    target = WORKFLOW_PATH
    source = read(target)
    mutated = source.replace(
        "ci/check_kagemusha_recursive_spend_payload_bench.sh --negative-control-transition-profile-growth",
        "ci/check_kagemusha_recursive_spend_payload_bench.sh --synthetic-transition-profile-check",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate payload reducer negative controls")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected payload reducer negative-control workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: payload reducer negative-control drift was not detected")

if mode == "--negative-control-reserved-lineage-payload-negative-controls-workflow":
    target = WORKFLOW_PATH
    source = read(target)
    mutated = source.replace(
        "ci/check_kagemusha_recursive_spend_payload_bench.sh --negative-control-reserved-lineage-payload-growth",
        "ci/check_kagemusha_recursive_spend_payload_bench.sh --synthetic-reserved-lineage-payload-check",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate Reserved-lineage payload reducer negative controls")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected Reserved-lineage payload reducer workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Reserved-lineage payload reducer drift was not detected")

if mode == "--negative-control-payload-negative-controls-comment-workflow":
    target = WORKFLOW_PATH
    source = read(target)
    mutated = source.replace(
        "          ci/check_kagemusha_recursive_spend_payload_bench.sh --negative-control-transition-profile-growth",
        "          # ci/check_kagemusha_recursive_spend_payload_bench.sh --negative-control-transition-profile-growth",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to comment payload reducer negative-control command")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected commented payload reducer command drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: commented payload reducer command drift was not detected")

if mode == "--negative-control-payload-negative-controls-order-workflow":
    target = WORKFLOW_PATH
    source = read(target)
    command = (
        "          ci/check_kagemusha_recursive_spend_payload_bench.sh "
        "--negative-control-transition-profile-growth"
    )
    mutated = source.replace(f"{command}\n", "", 1)
    mutated = mutated.replace(
        "        run: ci/check_kagemusha_recursive_spend_payload_bench.sh\n",
        f"        run: ci/check_kagemusha_recursive_spend_payload_bench.sh\n{command}\n",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to move payload reducer negative-control command after benchmark")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected payload reducer negative-control ordering drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: payload reducer negative-control ordering drift was not detected")

if mode == "--negative-control-payload-benchmark-workflow":
    target = WORKFLOW_PATH
    source = read(target)
    mutated = source.replace(
        '      - "crates/iroha_data_model/benches/kagemusha_recursive_spend_payload.rs"\n',
        "",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate payload benchmark workflow path")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected payload benchmark workflow path drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: payload benchmark workflow path drift was not detected")

if mode == "--negative-control-payload-benchmark-manifest-workflow":
    target = WORKFLOW_PATH
    source = read(target)
    mutated = source.replace(
        '      - "crates/iroha_data_model/Cargo.toml"\n',
        "",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate payload benchmark manifest workflow path")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected payload benchmark manifest workflow path drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: payload benchmark manifest workflow path drift was not detected")

if mode == "--negative-control-payload-benchmark-source":
    target = "crates/iroha_data_model/benches/kagemusha_recursive_spend_payload.rs"
    source = read(target)
    mutated = source.replace(
        "kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight",
        "kagemusha_recursive_spend_transition_profile_append_evidence_without_opening_preflight",
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate payload benchmark source coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected payload benchmark source drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: payload benchmark source drift was not detected")

if mode == "--negative-control-doc-payload-budget":
    target = "docs/source/offline_kagemusha.md"
    source = read(target)
    mutated = source.replace("1,751 bytes", "1,553 bytes", 1)
    mutated = mutated.replace(
        "`kagemusha_reserved_lineage_transition_profile_bytes`",
        "`kagemusha_recursive_spend_reserved_lineage_transition_profile_bytes`",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate documented payload budget")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected documented payload budget drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: documented payload budget drift was not detected")

if mode == "--negative-control-doc-sdk-host-boundary":
    target = "docs/source/offline_kagemusha.md"
    source = read(target)
    mutated = source.replace(
        "SDK append wrappers validate the metadata tuple",
        "C bridge, JavaScript NAPI host, and Python PyO3 host validate the metadata tuple",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate documented SDK host boundary")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected documented SDK host-boundary drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: documented SDK host-boundary drift was not detected")

if mode == "--negative-control-doc-sdk-availability-surface":
    target = "docs/source/offline_kagemusha.md"
    source = read(target)
    mutated = source.replace("the append-boundary helper, ", "", 1)
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate documented SDK availability surface")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected documented SDK availability surface drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: documented SDK availability surface drift was not detected")

if mode == "--negative-control-doc-abi-entry-count":
    target = "docs/source/offline_kagemusha.md"
    source = read(target)
    mutated = source.replace("All nine entry points accept", "All eight entry points accept", 1)
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate documented ABI-6 entry count")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected documented ABI-6 entry-count drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: documented ABI-6 entry-count drift was not detected")

if mode == "--negative-control-roadmap-abi-surface":
    target = "roadmap.md"
    source = read(target)
    mutated = source.replace(
        "both transition-profile\n  helpers, append-boundary derivation, both lineage-witness assembly helpers",
        "lineage-witness assembly",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate roadmap ABI-6 surface")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected roadmap ABI-6 surface drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: roadmap ABI-6 surface drift was not detected")

if mode == "--negative-control-policy-negative-controls-workflow":
    target = WORKFLOW_PATH
    source = read(target)
    mutated = source.replace(
        "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-redeem-order",
        "ci/check_kagemusha_recursive_spend_policy.sh --synthetic-core-redeem-order-check",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate policy negative-control workflow command")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected policy negative-control workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: policy negative-control workflow drift was not detected")

if mode == "--negative-control-policy-negative-controls-comment-workflow":
    target = WORKFLOW_PATH
    source = read(target)
    mutated = source.replace(
        "          ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-redeem-order",
        "          # ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-redeem-order",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to comment policy negative-control command")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected commented policy command drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: commented policy command drift was not detected")

if mode == "--negative-control-policy-negative-controls-order-workflow":
    target = WORKFLOW_PATH
    source = read(target)
    command = "          ci/check_kagemusha_recursive_spend_policy.sh --negative-control-core-redeem-order"
    mutated = source.replace(f"{command}\n", "", 1)
    mutated = mutated.replace(
        f"        run: {POLICY_MAIN_COMMAND}",
        f"        run: {POLICY_MAIN_COMMAND}\n{command}",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to move policy negative-control command after main guard")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected policy negative-control ordering drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: policy negative-control ordering drift was not detected")

if mode == "--negative-control-header-negative-controls-workflow":
    target = WORKFLOW_PATH
    source = read(target)
    mutated = source.replace(
        "ci/check_connect_norito_bridge_header.sh --negative-control-bad-recursive-signature",
        "ci/check_connect_norito_bridge_header.sh --synthetic-bad-recursive-signature-check",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate header negative-control workflow command")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected header negative-control workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: header negative-control workflow drift was not detected")

if mode == "--negative-control-python-sdk-test-workflow":
    target = WORKFLOW_PATH
    source = read(target)
    mutated = source.replace(
        f"        run: {PYTHON_SDK_TEST_COMMAND}",
        f"        run: {PYTHON_SDK_TEST_COMMAND} --skip",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate Python SDK test workflow command")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected Python SDK test workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK test workflow drift was not detected")

if mode == "--negative-control-core-isi":
    target = "crates/iroha_core/src/smartcontracts/isi/offline.rs"
    source = read(target)
    mutated = source.replace(
        "fn kagemusha_recursive_redeem_rejects_semantic_recursive_spend_before_mint",
        "fn kagemusha_recursive_redeem_missing_fail_closed_test",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate core ISI coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected core ISI redemption coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: core ISI coverage drift was not detected")

if mode == "--negative-control-core-multi-hop-redeem-success":
    target = "crates/iroha_core/src/smartcontracts/isi/offline.rs"
    source = read(target)
    mutated = source.replace(
        "fn kagemusha_recursive_redeem_record_backed_multi_hop_mints_and_rejects_replay",
        "fn kagemusha_recursive_redeem_missing_multi_hop_success_coverage",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate multi-hop redeem success coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected multi-hop redeem success coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: multi-hop redeem success coverage drift was not detected")

if mode == "--negative-control-core-lineage-hop-proof":
    target = "crates/iroha_core/src/smartcontracts/isi/offline.rs"
    source = read(target)
    mutated = source.replace(
        "fn kagemusha_recursive_redeem_rejects_malformed_lineage_hop_proof_before_mint",
        "fn kagemusha_recursive_redeem_missing_malformed_lineage_hop_coverage",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate malformed lineage hop proof coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected malformed lineage hop proof coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: malformed lineage hop proof coverage drift was not detected")

if mode == "--negative-control-core-redeem-order":
    target = "crates/iroha_core/src/smartcontracts/isi/offline.rs"
    source = read(target)
    mutated = source.replace(
        "state_transaction.register_confidential_proof(self.redeem_proof.proof.bytes.len())",
        "state_transaction.register_confidential_final_redeem_proof_after_mint(self.redeem_proof.proof.bytes.len())",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate core redeem execution order")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected core redeem execution-order drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: core redeem execution-order drift was not detected")

if mode == "--negative-control-core-redeem-early-mint":
    target = "crates/iroha_core/src/smartcontracts/isi/offline.rs"
    source = read(target)
    mutated = source.replace(
        "            self.bundle\n                .validate_public_input_binding()",
        "            let _unguarded_mint = Mint::asset_numeric(\n"
        "                Numeric::new(self.public_amount, 0),\n"
        "                AssetId::of(self.bundle.accumulator.asset.clone(), self.recipient.clone()),\n"
        "            );\n"
        "            self.bundle\n                .validate_public_input_binding()",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to inject early core redeem mint")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected early core redeem mint drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: early core redeem mint drift was not detected")

if mode == "--negative-control-status-doc-drift":
    target = "status.md"
    source = read(target)
    mutated = source.replace(
        "witnessless chain redemption is not\n  admitted",
        "witnessless chain redemption is admitted\n  only inside the wired one-hop verifier-slice bound",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate status documentation boundary")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected status documentation drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: status documentation drift was not detected")

if mode == "--negative-control-workflow-cancel-in-progress":
    target = WORKFLOW_PATH
    source = read(target)
    mutated = source.replace(
        "  cancel-in-progress: false",
        "  cancel-in-progress: true",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate workflow cancellation policy")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected workflow cancellation drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: workflow cancellation drift was not detected")

if mode == "--negative-control-main-guards-workflow":
    target = WORKFLOW_PATH
    source = read(target)
    mutated = source.replace(
        "        run: ci/check_kagemusha_recursive_spend_sdk_parity.sh",
        "        run: ci/check_kagemusha_recursive_spend_sdk_parity.sh --skip-main-guard",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate main guard workflow command")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected main guard workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: main guard workflow drift was not detected")

if mode == "--negative-control-verify-result-flags":
    target = "crates/iroha_core/src/zk.rs"
    source = read(target)
    mutated = source.replace(
        "let lineage_witness_required_for_redeem = !witnessless_redeem_supported;",
        "let lineage_witness_required_for_redeem = false;",
        1,
    )
    if mutated == source:
        raise SystemExit("negative control failed: unable to mutate verify-result fail-closed flags")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PolicyError as error:
        print("negative control rejected verify-result fail-closed flag drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: verify-result flag drift was not detected")

if mode:
    raise SystemExit(f"unknown mode: {mode}")

try:
    run_checks()
except PolicyError as error:
    raise SystemExit(str(error))

print("recursive Kagemusha witnessless Reserved-lineage policy is enabled to 64 hops")
PY
