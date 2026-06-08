// NoritoBridge C FFI header
// Place this header into the XCFramework at: NoritoBridge.xcframework/**/Headers/connect_norito_bridge.h
// And include a modulemap at: NoritoBridge.xcframework/**/Modules/module.modulemap
//
// module.modulemap example:
//   module NoritoBridge { header "connect_norito_bridge.h" export * }

#ifndef CONNECT_NORITO_BRIDGE_H
#define CONNECT_NORITO_BRIDGE_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define CONNECT_NORITO_ERR_ACCOUNT_ADDRESS -200
#define CONNECT_NORITO_ERR_UNSUPPORTED_ALGORITHM -21
#define CONNECT_NORITO_ERR_OFFLINE_RECEIVER -300
#define CONNECT_NORITO_ERR_OFFLINE_ASSET -301
#define CONNECT_NORITO_ERR_OFFLINE_NONCE -303
#define CONNECT_NORITO_ERR_OFFLINE_SERIALIZE -304
#define CONNECT_NORITO_ERR_OFFLINE_NOTE_PROVE -310
#define CONNECT_NORITO_ERR_KAGEMUSHA_PROVE -311
#define CONNECT_NORITO_ERR_KAGEMUSHA_RECURSIVE_COMPACT_UNAVAILABLE -312

// ---------------- Bridge ABI ----------------
uint32_t connect_norito_bridge_abi_version(void);

// ---------------- Chain discriminant helpers ----------------
uint16_t connect_norito_get_chain_discriminant(void);
uint16_t connect_norito_set_chain_discriminant(uint16_t discriminant);

// ---------------- Account address helpers ----------------
int32_t connect_norito_account_address_parse(
    const char* input,
    unsigned long input_len,
    uint16_t expected_prefix,
    uint8_t expected_prefix_present,
    uint8_t** out_canonical_ptr,
    unsigned long* out_canonical_len,
    uint16_t* out_network_prefix,
    uint8_t** out_error_json_ptr,
    unsigned long* out_error_json_len);

int32_t connect_norito_account_address_render(
    const uint8_t* canonical_ptr,
    unsigned long canonical_len,
    uint16_t network_prefix,
    uint8_t** out_canonical_hex_ptr,
    unsigned long* out_canonical_hex_len,
    uint8_t** out_i105_ptr,
    unsigned long* out_i105_len,
    uint8_t** out_error_json_ptr,
    unsigned long* out_error_json_len);

// ---------------- Ciphertext frame ----------------
int32_t connect_norito_encode_ciphertext_frame(
    const uint8_t* sid, uint8_t dir, uint64_t seq,
    const uint8_t* aead, unsigned long aead_len,
    uint8_t** out_ptr, unsigned long* out_len);

int32_t connect_norito_decode_ciphertext_frame(
    const uint8_t* inp, unsigned long inp_len,
    uint8_t* out_sid, uint8_t* out_dir, uint64_t* out_seq,
    uint8_t** out_aead_ptr, unsigned long* out_aead_len);

// ---------------- Offline Note prover helpers ----------------
// Generate a recursive Halo2/IPA proof for an Offline redemption.
// Input: Norito-archive bytes of `OfflineNoteRedeem` (recursive_proof field is ignored).
// Output: Norito-archive bytes of `OfflineNoteRecursiveProof` (slot back into the redemption).
int32_t connect_norito_offline_prove_note_redeem(
    const uint8_t* redeem_norito_ptr,
    unsigned long redeem_norito_len,
    uint8_t** out_recursive_proof_ptr,
    unsigned long* out_recursive_proof_len);

// Generate a recursive Halo2/IPA proof for an Offline audit bundle.
// Input: Norito-archive bytes of `OfflineNoteAuditBundle` (recursive_proof field is ignored).
// Output: Norito-archive bytes of `OfflineNoteRecursiveProof` (slot back into the audit bundle).
int32_t connect_norito_offline_prove_note_audit(
    const uint8_t* audit_norito_ptr,
    unsigned long audit_norito_len,
    uint8_t** out_recursive_proof_ptr,
    unsigned long* out_recursive_proof_len);

// Encode and sign a `RedeemOfflineNote` on-chain transaction.
// `redeem_norito` is the Norito archive of `OfflineNoteRedeem` with the
// recursive proof already embedded. Output is canonical versioned
// SignedTransaction bytes matching the transfer/mint encoders below.
int32_t connect_norito_encode_redeem_offline_note_signed_transaction(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    uint32_t nonce,
    uint8_t nonce_present,
    const uint8_t* redeem_norito_ptr, unsigned long redeem_norito_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

// Encode and sign an `AuditOfflineNote` on-chain transaction.
// `audit_norito` is the Norito archive of `OfflineNoteAuditBundle` with the
// recursive proof already embedded.
int32_t connect_norito_encode_audit_offline_note_signed_transaction(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    uint32_t nonce,
    uint8_t nonce_present,
    const uint8_t* audit_norito_ptr, unsigned long audit_norito_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

// Encode and sign an `IssueOfflineNote` on-chain transaction.
// `issue_norito` is the Norito archive of `OfflineNoteIssue`.
int32_t connect_norito_encode_issue_offline_note_signed_transaction(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    uint32_t nonce,
    uint8_t nonce_present,
    const uint8_t* issue_norito_ptr, unsigned long issue_norito_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

// Encode and sign a defund transaction: bearer audit trail followed by the
// redemption, atomically in one signed transaction. `audit_trail` is a
// count-prefixed concatenation. Each entry is an 8-byte little-endian length
// followed by that many bytes of an `OfflineNoteAuditBundle` archive.
int32_t connect_norito_encode_defund_offline_note_signed_transaction(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    uint32_t nonce,
    uint8_t nonce_present,
    const uint8_t* audit_trail_ptr, unsigned long audit_trail_len, uint32_t audit_trail_count,
    const uint8_t* redeem_norito_ptr, unsigned long redeem_norito_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

// Legacy unanchored Kagemusha compact-token prover retained for ABI compatibility only.
// Production callers must use `connect_norito_kagemusha_prove_verified_compact_payment_token_with_records`.
// Valid `KagemushaVerifiedFoldBundle` input returns ERR_KAGEMUSHA_PROVE and no output bytes.
int32_t connect_norito_kagemusha_prove_verified_compact_payment_token(
    const uint8_t* verified_bundle_norito_ptr,
    unsigned long verified_bundle_norito_len,
    uint8_t** out_compact_token_ptr,
    unsigned long* out_compact_token_len);

// Verify private Kagemusha hop proofs against verifier records and generate a compact folded-token proof.
// Input: Norito-archive bytes of `KagemushaVerifiedFoldRecordBundle`.
// Output: Norito-archive bytes of `KagemushaCompactPaymentToken`.
int32_t connect_norito_kagemusha_prove_verified_compact_payment_token_with_records(
    const uint8_t* verified_record_bundle_norito_ptr,
    unsigned long verified_record_bundle_norito_len,
    uint8_t** out_compact_token_ptr,
    unsigned long* out_compact_token_len);

// Verify private Kagemusha hop proofs plus proof-derived Pallas opening envelopes and
// generate an admission-neutral recursive aggregation proof bundle.
// Input 1: Norito-archive bytes of `KagemushaVerifiedFoldRecordBundle`.
// Input 2: Norito-archive bytes of `Vec<iroha_data_model::zk::OpenVerifyEnvelope>`.
// Output: Norito-archive bytes of `KagemushaRecursiveAggregationProofBundle`.
int32_t connect_norito_kagemusha_prove_verified_recursive_aggregation_proof_bundle_with_records_and_pallas_open_envelopes(
    const uint8_t* verified_record_bundle_norito_ptr,
    unsigned long verified_record_bundle_norito_len,
    const uint8_t* pallas_open_envelopes_norito_ptr,
    unsigned long pallas_open_envelopes_norito_len,
    uint8_t** out_proof_bundle_ptr,
    unsigned long* out_proof_bundle_len);

// ABI 7 recursive compact-token prover surface for `kagemusha-recursive-compact-v1`.
// Reserved source-stable surface. It rejects malformed input archives and fails
// closed with ERR_KAGEMUSHA_RECURSIVE_COMPACT_UNAVAILABLE until compact-token
// proofs compose the private-hop verifier-slice relation in-circuit.
// Input 1: Norito-archive bytes of `KagemushaVerifiedFoldRecordBundle`.
// Input 2: Norito-archive bytes of `Vec<iroha_data_model::zk::OpenVerifyEnvelope>`.
// Output: none in this release.
int32_t connect_norito_kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes(
    const uint8_t* verified_record_bundle_norito_ptr,
    unsigned long verified_record_bundle_norito_len,
    const uint8_t* pallas_open_envelopes_norito_ptr,
    unsigned long pallas_open_envelopes_norito_len,
    uint8_t** out_compact_token_ptr,
    unsigned long* out_compact_token_len);

// Verify an ABI 7 recursive compact Kagemusha token.
// Input: Norito-archive bytes of `KagemushaCompactPaymentToken`.
// Malformed archives and malformed token bindings return ERR_KAGEMUSHA_PROVE.
// Output: `*out_valid = 0` for every shape-valid token in this release.
int32_t connect_norito_kagemusha_verify_recursive_compact_payment_token(
    const uint8_t* compact_token_norito_ptr,
    unsigned long compact_token_norito_len,
    uint8_t* out_valid);

// Verify a projected recursive spend compact Kagemusha token against a lineage verifier record.
// Input 1: Norito-archive bytes of `KagemushaCompactPaymentToken`.
// Input 2: Norito-archive bytes of `VerifyingKeyRecord`.
// Malformed archives, non-lineage circuit ids, inactive/windowed records without
// a height, and malformed token bindings return ERR_KAGEMUSHA_PROVE.
// Output: `*out_valid = 1` only when the projected token verifies.
int32_t connect_norito_kagemusha_verify_recursive_spend_compact_payment_token_projection(
    const uint8_t* compact_token_norito_ptr,
    unsigned long compact_token_norito_len,
    const uint8_t* verifier_record_norito_ptr,
    unsigned long verifier_record_norito_len,
    uint8_t* out_valid);

// Verify a projected recursive spend compact Kagemusha token at `block_height`.
int32_t connect_norito_kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height(
    const uint8_t* compact_token_norito_ptr,
    unsigned long compact_token_norito_len,
    const uint8_t* verifier_record_norito_ptr,
    unsigned long verifier_record_norito_len,
    uint64_t block_height,
    uint8_t* out_valid);

// Project a recursive spend bundle into an ABI 7 recursive compact Kagemusha token.
// Input: Norito-archive bytes of `KagemushaRecursiveSpendBundleV1`.
// Output: Norito-archive bytes of `KagemushaCompactPaymentToken`.
int32_t connect_norito_kagemusha_recursive_spend_compact_payment_token_from_bundle(
    const uint8_t* bundle_norito_ptr,
    unsigned long bundle_norito_len,
    uint8_t** out_compact_token_ptr,
    unsigned long* out_compact_token_len);

// Initialize production recursive Kagemusha spendable offline cash.
// Input: Norito-archive bytes of `KagemushaRecursiveSpendInitRequestV1`.
// Output: Norito-archive bytes of `KagemushaRecursiveSpendBundleV1`.
int32_t connect_norito_kagemusha_recursive_spend_init(
    const uint8_t* request_norito_ptr,
    unsigned long request_norito_len,
    uint8_t** out_bundle_ptr,
    unsigned long* out_bundle_len);

// Append one offline hop to production recursive Kagemusha spendable cash.
// Input: Norito-archive bytes of `KagemushaRecursiveSpendAppendRequestV1`.
// Output: Norito-archive bytes of `KagemushaRecursiveSpendBundleV1`.
int32_t connect_norito_kagemusha_recursive_spend_append(
    const uint8_t* request_norito_ptr,
    unsigned long request_norito_len,
    uint8_t** out_bundle_ptr,
    unsigned long* out_bundle_len);

// Build the canonical initial Reserved-lineage accumulator transition profile.
// Input: Norito-archive bytes of `KagemushaRecursiveSpendInitRequestV1`.
// Output: Norito-archive bytes of `KagemushaRecursiveSpendTransitionProfileV1`.
int32_t connect_norito_kagemusha_recursive_spend_transition_profile_init(
    const uint8_t* request_norito_ptr,
    unsigned long request_norito_len,
    uint8_t** out_profile_ptr,
    unsigned long* out_profile_len);

// Build the canonical append Reserved-lineage accumulator transition profile.
// Input: Norito-archive bytes of `KagemushaRecursiveSpendAppendRequestV1`.
// Output: Norito-archive bytes of `KagemushaRecursiveSpendTransitionProfileV1`.
int32_t connect_norito_kagemusha_recursive_spend_transition_profile_append(
    const uint8_t* request_norito_ptr,
    unsigned long request_norito_len,
    uint8_t** out_profile_ptr,
    unsigned long* out_profile_len);

// Build the compact Reserved-lineage append boundary from a transition profile.
// Input: Norito-archive bytes of `KagemushaRecursiveSpendTransitionProfileV1`.
// Output: Norito-archive bytes of `KagemushaRecursiveSpendLineageAppendBoundaryV1`.
int32_t connect_norito_kagemusha_recursive_spend_lineage_append_boundary(
    const uint8_t* profile_norito_ptr,
    unsigned long profile_norito_len,
    uint8_t** out_boundary_ptr,
    unsigned long* out_boundary_len);

// Build the initial recursive spend lineage witness.
// Inputs: Norito-archive bytes of `KagemushaRecursiveSpendInitRequestV1`
// and the resulting `KagemushaRecursiveSpendBundleV1`.
// Output: Norito-archive bytes of `KagemushaRecursiveSpendLineageWitnessV1`.
int32_t connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result(
    const uint8_t* request_norito_ptr,
    unsigned long request_norito_len,
    const uint8_t* bundle_norito_ptr,
    unsigned long bundle_norito_len,
    uint8_t** out_witness_ptr,
    unsigned long* out_witness_len);

// Append one hop of recursive spend lineage witness material.
// Inputs: Norito-archive bytes of the previous
// `KagemushaRecursiveSpendLineageWitnessV1`, the
// `KagemushaRecursiveSpendAppendRequestV1`, and the resulting
// `KagemushaRecursiveSpendBundleV1`.
// Output: Norito-archive bytes of the appended lineage witness.
int32_t connect_norito_kagemusha_recursive_spend_lineage_witness_append_result(
    const uint8_t* previous_witness_norito_ptr,
    unsigned long previous_witness_norito_len,
    const uint8_t* request_norito_ptr,
    unsigned long request_norito_len,
    const uint8_t* bundle_norito_ptr,
    unsigned long bundle_norito_len,
    uint8_t** out_witness_ptr,
    unsigned long* out_witness_len);

// Verify production recursive Kagemusha spendable offline cash.
// Input: Norito-archive bytes of `KagemushaRecursiveSpendVerifyRequestV1`.
// Output: Norito-archive bytes of `KagemushaRecursiveSpendVerifyResultV1`.
// Malformed request archives, including missing or forged Reserved-lineage
// verifier records and unsupported proof attachments, return
// ERR_KAGEMUSHA_PROVE instead of a diagnostic result archive.
int32_t connect_norito_kagemusha_recursive_spend_verify(
    const uint8_t* request_norito_ptr,
    unsigned long request_norito_len,
    uint8_t** out_result_ptr,
    unsigned long* out_result_len);

// Prepare an online recursive Kagemusha redeem instruction.
// Input: Norito-archive bytes of `KagemushaRecursiveSpendRedeemRequestV1`.
// Output: Norito-archive bytes of `RedeemKagemushaRecursive`.
int32_t connect_norito_kagemusha_recursive_spend_redeem(
    const uint8_t* request_norito_ptr,
    unsigned long request_norito_len,
    uint8_t** out_instruction_ptr,
    unsigned long* out_instruction_len);

void connect_norito_free(uint8_t* ptr);

// ---------------- Privacy proof native FFI ----------------
// Output buffers are Norito V1 archives allocated by the bridge and must be
// released with `iroha_privacy_free_buffer`, which zeroizes privacy output
// memory before release.
int32_t iroha_privacy_capabilities_v1(
    uint8_t** out_ptr,
    unsigned long* out_len);

int32_t iroha_privacy_build_proof_v1(
    const uint8_t* request_ptr,
    unsigned long request_len,
    uint8_t** out_ptr,
    unsigned long* out_len);

int32_t iroha_privacy_verify_proof_v1(
    const uint8_t* request_ptr,
    unsigned long request_len,
    uint8_t** out_ptr,
    unsigned long* out_len);

void iroha_privacy_free_buffer(uint8_t* ptr);

// ---------------- Envelope helpers ----------------
int32_t connect_norito_encode_envelope_sign_request_tx(
    uint64_t seq,
    const uint8_t* tx, unsigned long tx_len,
    uint8_t** out_ptr, unsigned long* out_len);

int32_t connect_norito_encode_envelope_sign_request_raw(
    uint64_t seq,
    const uint8_t* tag, unsigned long tag_len,
    const uint8_t* bytes, unsigned long bytes_len,
    uint8_t** out_ptr, unsigned long* out_len);

int32_t connect_norito_encode_envelope_sign_result_ok(
    uint64_t seq,
    const uint8_t* sig, unsigned long sig_len,
    uint8_t** out_ptr, unsigned long* out_len);

int32_t connect_norito_encode_envelope_sign_result_err(
    uint64_t seq,
    const uint8_t* code, unsigned long code_len,
    const uint8_t* message, unsigned long message_len,
    uint8_t** out_ptr, unsigned long* out_len);

// ---------------- Signing helpers ----------------
int32_t connect_norito_public_key_from_private(
    uint8_t algorithm_code,
    const uint8_t* private_key_ptr,
    unsigned long private_key_len,
    uint8_t** out_public_key_ptr,
    unsigned long* out_public_key_len);

int32_t connect_norito_keypair_from_seed(
    uint8_t algorithm_code,
    const uint8_t* seed_ptr,
    unsigned long seed_len,
    uint8_t** out_private_key_ptr,
    unsigned long* out_private_key_len,
    uint8_t** out_public_key_ptr,
    unsigned long* out_public_key_len);

int32_t connect_norito_sign_detached(
    uint8_t algorithm_code,
    const uint8_t* private_key_ptr,
    unsigned long private_key_len,
    const uint8_t* message_ptr,
    unsigned long message_len,
    uint8_t** out_signature_ptr,
    unsigned long* out_signature_len);

int32_t connect_norito_verify_detached(
    uint8_t algorithm_code,
    const uint8_t* public_key_ptr,
    unsigned long public_key_len,
    const uint8_t* message_ptr,
    unsigned long message_len,
    const uint8_t* signature_ptr,
    unsigned long signature_len,
    uint8_t* out_valid);

// ---------------- Secp256k1 helpers ----------------
int32_t connect_norito_secp256k1_public_key(
    const uint8_t* private_key,
    unsigned long private_key_len,
    uint8_t* out_public_key,
    unsigned long out_public_key_len);

int32_t connect_norito_secp256k1_sign(
    const uint8_t* private_key,
    unsigned long private_key_len,
    const uint8_t* message,
    unsigned long message_len,
    uint8_t* out_signature,
    unsigned long out_signature_len);

int32_t connect_norito_secp256k1_verify(
    const uint8_t* public_key,
    unsigned long public_key_len,
    const uint8_t* message,
    unsigned long message_len,
    const uint8_t* signature,
    unsigned long signature_len);

// ---------------- SM2 helpers ----------------
int32_t connect_norito_sm2_default_distid(
    uint8_t** out_ptr,
    unsigned long* out_len);

int32_t connect_norito_sm2_keypair_from_seed(
    const char* distid,
    unsigned long distid_len,
    const uint8_t* seed,
    unsigned long seed_len,
    uint8_t* out_private,
    unsigned long out_private_len,
    uint8_t* out_public,
    unsigned long out_public_len);

int32_t connect_norito_sm2_sign(
    const char* distid,
    unsigned long distid_len,
    const uint8_t* private_key,
    unsigned long private_key_len,
    const uint8_t* message,
    unsigned long message_len,
    uint8_t* out_signature,
    unsigned long out_signature_len);

int32_t connect_norito_sm2_verify(
    const char* distid,
    unsigned long distid_len,
    const uint8_t* public_key,
    unsigned long public_key_len,
    const uint8_t* message,
    unsigned long message_len,
    const uint8_t* signature,
    unsigned long signature_len);

int32_t connect_norito_sm2_public_key_prefixed(
    const char* distid,
    unsigned long distid_len,
    const uint8_t* public_key,
    unsigned long public_key_len,
    uint8_t** out_ptr,
    unsigned long* out_len);

int32_t connect_norito_sm2_public_key_multihash(
    const char* distid,
    unsigned long distid_len,
    const uint8_t* public_key,
    unsigned long public_key_len,
    uint8_t** out_ptr,
    unsigned long* out_len);

int32_t connect_norito_sm2_compute_za(
    const char* distid,
    unsigned long distid_len,
    const uint8_t* public_key,
    unsigned long public_key_len,
    uint8_t* out_za,
    unsigned long out_za_len);

// ---------------- SoraFS helpers ----------------
int32_t connect_norito_sorafs_local_fetch(
    const char* plan_json,
    unsigned long plan_len,
    const char* providers_json,
    unsigned long providers_len,
    const char* options_json,
    unsigned long options_len,
    uint8_t** out_payload_ptr,
    unsigned long* out_payload_len,
    uint8_t** out_report_ptr,
    unsigned long* out_report_len);

// ---------------- DA proof helpers ----------------
int32_t connect_norito_da_proof_summary(
    const uint8_t* manifest_ptr,
    unsigned long manifest_len,
    const uint8_t* payload_ptr,
    unsigned long payload_len,
    unsigned long sample_count,
    uint64_t sample_seed,
    const unsigned long* leaf_indexes_ptr,
    unsigned long leaf_indexes_len,
    uint8_t** out_json_ptr,
    unsigned long* out_json_len);

int32_t connect_norito_encode_envelope_control_close(
    uint64_t seq, uint8_t who, uint16_t code,
    const uint8_t* reason, unsigned long reason_len,
    uint8_t retryable,
    uint8_t** out_ptr, unsigned long* out_len);

// ---------------- Hash helpers ----------------
int32_t connect_norito_blake3_hash(
    const uint8_t* payload_ptr,
    unsigned long payload_len,
    uint8_t** out_digest_ptr,
    unsigned long* out_digest_len);

int32_t connect_norito_encode_envelope_control_reject(
    uint64_t seq, uint16_t code,
    const uint8_t* code_id, unsigned long code_id_len,
    const uint8_t* reason, unsigned long reason_len,
    uint8_t** out_ptr, unsigned long* out_len);

int32_t connect_norito_decode_envelope_kind(
    const uint8_t* inp, unsigned long inp_len,
    uint64_t* out_seq, uint16_t* out_kind);

int32_t connect_norito_decode_envelope_json(
    const uint8_t* inp, unsigned long inp_len,
    uint8_t** out_ptr, unsigned long* out_len);

// ---------------- Control decode helpers ----------------
int32_t connect_norito_decode_control_kind(
    const uint8_t* inp, unsigned long inp_len,
    uint8_t* out_sid, uint8_t* out_dir, uint64_t* out_seq, uint16_t* out_kind);

int32_t connect_norito_decode_control_open_pub(
    const uint8_t* inp, unsigned long inp_len,
    uint8_t* out_pk);

int32_t connect_norito_decode_control_approve_pub(
    const uint8_t* inp, unsigned long inp_len,
    uint8_t* out_pk);

int32_t connect_norito_decode_control_approve_account(
    const uint8_t* inp, unsigned long inp_len,
    uint8_t** out_ptr, unsigned long* out_len);

int32_t connect_norito_decode_control_approve_sig(
    const uint8_t* inp, unsigned long inp_len,
    uint8_t* out_sig); // 64 bytes

int32_t connect_norito_decode_control_approve_account_json(
    const uint8_t* inp, unsigned long inp_len,
    uint8_t** out_ptr, unsigned long* out_len);

int32_t connect_norito_decode_control_close(
    const uint8_t* inp, unsigned long inp_len,
    uint8_t* out_who, uint16_t* out_code, uint8_t* out_retryable,
    uint8_t** out_reason_ptr, unsigned long* out_reason_len);

int32_t connect_norito_decode_control_reject(
    const uint8_t* inp, unsigned long inp_len,
    uint16_t* out_code,
    uint8_t** out_code_id_ptr, unsigned long* out_code_id_len,
    uint8_t** out_reason_ptr, unsigned long* out_reason_len);

int32_t connect_norito_decode_control_ping(
    const uint8_t* inp, unsigned long inp_len,
    uint64_t* out_nonce);

int32_t connect_norito_decode_control_pong(
    const uint8_t* inp, unsigned long inp_len,
    uint64_t* out_nonce);

// ---------------- Permissions/Proof JSON ----------------
int32_t connect_norito_decode_control_open_app_metadata_json(
    const uint8_t* inp, unsigned long inp_len,
    uint8_t** out_ptr, unsigned long* out_len);

int32_t connect_norito_decode_control_open_permissions_json(
    const uint8_t* inp, unsigned long inp_len,
    uint8_t** out_ptr, unsigned long* out_len);

int32_t connect_norito_decode_control_open_chain_id(
    const uint8_t* inp, unsigned long inp_len,
    uint8_t** out_ptr, unsigned long* out_len);

int32_t connect_norito_decode_control_approve_permissions_json(
    const uint8_t* inp, unsigned long inp_len,
    uint8_t** out_ptr, unsigned long* out_len);

int32_t connect_norito_decode_control_approve_proof_json(
    const uint8_t* inp, unsigned long inp_len,
    uint8_t** out_ptr, unsigned long* out_len);

// ---------------- Extended control encoders ----------------
int32_t connect_norito_encode_control_open_ext(
    const uint8_t* sid,
    uint8_t dir,
    uint64_t seq,
    const uint8_t* app_pk, unsigned long app_pk_len,
    const uint8_t* app_meta_json, unsigned long app_meta_len,
    const char* chain_id,
    const uint8_t* permissions_json, unsigned long permissions_len,
    uint8_t** out_ptr, unsigned long* out_len);

int32_t connect_norito_encode_control_approve_ext(
    const uint8_t* sid,
    uint8_t dir,
    uint64_t seq,
    const uint8_t* wallet_pk, unsigned long wallet_pk_len,
    const char* account_id,
    const uint8_t* permissions_json, unsigned long permissions_len,
    const uint8_t* proof_json, unsigned long proof_len,
    const uint8_t* sig, unsigned long sig_len,
    uint8_t** out_ptr, unsigned long* out_len);

int32_t connect_norito_encode_control_approve_ext_with_alg(
    const uint8_t* sid,
    uint8_t dir,
    uint64_t seq,
    const uint8_t* wallet_pk,
    const char* account_id,
    unsigned long account_len,
    const char* permissions_json,
    unsigned long permissions_json_len,
    const char* proof_json,
    unsigned long proof_json_len,
    const char* alg,
    unsigned long alg_len,
    const uint8_t* sig,
    unsigned long sig_len,
    uint8_t** out_ptr,
    unsigned long* out_len);

int32_t connect_norito_encode_control_reject(
    const uint8_t* sid,
    uint8_t dir,
    uint64_t seq,
    uint16_t code,
    const char* code_id, unsigned long code_id_len,
    const char* reason, unsigned long reason_len,
    uint8_t** out_ptr, unsigned long* out_len);

int32_t connect_norito_encode_control_close(
    const uint8_t* sid,
    uint8_t dir,
    uint64_t seq,
    uint8_t who,
    uint16_t code,
    const char* reason, unsigned long reason_len,
    uint8_t retryable,
    uint8_t** out_ptr, unsigned long* out_len);

int32_t connect_norito_encode_control_ping(
    const uint8_t* sid,
    uint8_t dir,
    uint64_t seq,
    uint64_t nonce,
    uint8_t** out_ptr, unsigned long* out_len);

int32_t connect_norito_encode_control_pong(
    const uint8_t* sid,
    uint8_t dir,
    uint64_t seq,
    uint64_t nonce,
    uint8_t** out_ptr, unsigned long* out_len);

int32_t connect_norito_encode_confidential_encrypted_payload(
    const uint8_t* ephemeral_pubkey,
    unsigned long ephemeral_len,
    const uint8_t* nonce,
    unsigned long nonce_len,
    const uint8_t* ciphertext,
    unsigned long ciphertext_len,
    uint8_t** out_ptr, unsigned long* out_len);

// Transaction encoder error codes:
//   0  success
//  -1  null pointer provided for input/output
//  -2  invalid UTF-8 in input strings
//  -3  chain_id parse failure
//  -4  authority account id parse failure
//  -5  asset definition id parse failure
//  -6  destination account id parse failure
//  -7  quantity parse failure
//  -8  invalid TTL (zero when present)
//  -9  private key parse failure
// -10  allocation failure while writing output
// -11  provided hash buffer shorter than 32 bytes
// -31  invalid nonce (zero when present)
int32_t connect_norito_encode_transfer_signed_transaction(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    uint32_t nonce,
    uint8_t nonce_present,
    const char* asset_definition, unsigned long asset_definition_len,
    const char* quantity, unsigned long quantity_len,
    const char* destination, unsigned long destination_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_transfer_signed_transaction_with_fee_sponsor(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    uint32_t nonce,
    uint8_t nonce_present,
    const char* asset_definition, unsigned long asset_definition_len,
    const char* quantity, unsigned long quantity_len,
    const char* destination, unsigned long destination_len,
    const char* fee_sponsor, unsigned long fee_sponsor_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_transfer_signed_transaction_alg(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    uint32_t nonce,
    uint8_t nonce_present,
    const char* asset_definition, unsigned long asset_definition_len,
    const char* quantity, unsigned long quantity_len,
    const char* destination, unsigned long destination_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t algorithm,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_transfer_signed_transaction_with_fee_sponsor_alg(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    uint32_t nonce,
    uint8_t nonce_present,
    const char* asset_definition, unsigned long asset_definition_len,
    const char* quantity, unsigned long quantity_len,
    const char* destination, unsigned long destination_len,
    const char* fee_sponsor, unsigned long fee_sponsor_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t algorithm,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_shield_signed_transaction(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* asset_definition, unsigned long asset_definition_len,
    const char* from_account, unsigned long from_len,
    const char* amount, unsigned long amount_len,
    const uint8_t* note_commitment, unsigned long note_commitment_len,
    const uint8_t* payload_ephemeral, unsigned long payload_ephemeral_len,
    const uint8_t* payload_nonce, unsigned long payload_nonce_len,
    const uint8_t* payload_ciphertext, unsigned long payload_ciphertext_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_shield_signed_transaction_alg(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* asset_definition, unsigned long asset_definition_len,
    const char* from_account, unsigned long from_len,
    const char* amount, unsigned long amount_len,
    const uint8_t* note_commitment, unsigned long note_commitment_len,
    const uint8_t* payload_ephemeral, unsigned long payload_ephemeral_len,
    const uint8_t* payload_nonce, unsigned long payload_nonce_len,
    const uint8_t* payload_ciphertext, unsigned long payload_ciphertext_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t algorithm,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_unshield_signed_transaction(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* asset_definition, unsigned long asset_definition_len,
    const char* destination, unsigned long destination_len,
    const char* public_amount, unsigned long public_amount_len,
    const uint8_t* inputs, unsigned long inputs_len,
    const char* proof_json, unsigned long proof_json_len,
    const uint8_t* root_hint, unsigned long root_hint_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_unshield_signed_transaction_alg(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* asset_definition, unsigned long asset_definition_len,
    const char* destination, unsigned long destination_len,
    const char* public_amount, unsigned long public_amount_len,
    const uint8_t* inputs, unsigned long inputs_len,
    const char* proof_json, unsigned long proof_json_len,
    const uint8_t* root_hint, unsigned long root_hint_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t algorithm,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_zk_transfer_signed_transaction(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* asset_definition, unsigned long asset_definition_len,
    const uint8_t* inputs, unsigned long inputs_len,
    const uint8_t* outputs, unsigned long outputs_len,
    const char* proof_json, unsigned long proof_json_len,
    const uint8_t* root_hint, unsigned long root_hint_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_zk_transfer_signed_transaction_alg(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* asset_definition, unsigned long asset_definition_len,
    const uint8_t* inputs, unsigned long inputs_len,
    const uint8_t* outputs, unsigned long outputs_len,
    const char* proof_json, unsigned long proof_json_len,
    const uint8_t* root_hint, unsigned long root_hint_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t algorithm,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_claim_identifier_signed_transaction(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* account_id, unsigned long account_id_len,
    const char* receipt_json, unsigned long receipt_json_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_claim_identifier_signed_transaction_alg(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* account_id, unsigned long account_id_len,
    const char* receipt_json, unsigned long receipt_json_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t algorithm,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_set_key_value_signed_transaction(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    uint8_t target_kind,
    const char* object_id, unsigned long object_len,
    const char* key, unsigned long key_len,
    const uint8_t* value_json, unsigned long value_json_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_set_key_value_signed_transaction_alg(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    uint8_t target_kind,
    const char* object_id, unsigned long object_len,
    const char* key, unsigned long key_len,
    const uint8_t* value_json, unsigned long value_json_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t algorithm,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_remove_key_value_signed_transaction(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    uint8_t target_kind,
    const char* object_id, unsigned long object_len,
    const char* key, unsigned long key_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_remove_key_value_signed_transaction_alg(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    uint8_t target_kind,
    const char* object_id, unsigned long object_len,
    const char* key, unsigned long key_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t algorithm,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_governance_propose_deploy_signed_transaction(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* contract_address, unsigned long contract_address_len,
    const char* code_hash_hex, unsigned long code_hash_hex_len,
    const char* abi_hash_hex, unsigned long abi_hash_hex_len,
    const char* abi_version, unsigned long abi_version_len,
    uint64_t window_lower, uint64_t window_upper, uint8_t window_present,
    uint8_t mode_code, uint8_t mode_present,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_governance_propose_deploy_signed_transaction_alg(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* contract_address, unsigned long contract_address_len,
    const char* code_hash_hex, unsigned long code_hash_hex_len,
    const char* abi_hash_hex, unsigned long abi_hash_hex_len,
    const char* abi_version, unsigned long abi_version_len,
    uint64_t window_lower, uint64_t window_upper, uint8_t window_present,
    uint8_t mode_code, uint8_t mode_present,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t algorithm,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_governance_cast_plain_ballot_signed_transaction(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* referendum_id, unsigned long referendum_id_len,
    const char* owner, unsigned long owner_len,
    const char* amount, unsigned long amount_len,
    uint64_t duration_blocks,
    uint8_t direction,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_governance_cast_plain_ballot_signed_transaction_alg(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* referendum_id, unsigned long referendum_id_len,
    const char* owner, unsigned long owner_len,
    const char* amount, unsigned long amount_len,
    uint64_t duration_blocks,
    uint8_t direction,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t algorithm,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_governance_cast_zk_ballot_signed_transaction(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* election_id, unsigned long election_id_len,
    const char* proof_b64, unsigned long proof_b64_len,
    const uint8_t* public_inputs_json, unsigned long public_inputs_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_governance_cast_zk_ballot_signed_transaction_alg(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* election_id, unsigned long election_id_len,
    const char* proof_b64, unsigned long proof_b64_len,
    const uint8_t* public_inputs_json, unsigned long public_inputs_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t algorithm,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_governance_enact_referendum_signed_transaction(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* referendum_id_hex, unsigned long referendum_id_len,
    const char* preimage_hash_hex, unsigned long preimage_hash_len,
    uint64_t window_lower, uint64_t window_upper,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_governance_enact_referendum_signed_transaction_alg(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* referendum_id_hex, unsigned long referendum_id_len,
    const char* preimage_hash_hex, unsigned long preimage_hash_len,
    uint64_t window_lower, uint64_t window_upper,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t algorithm,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_governance_finalize_referendum_signed_transaction(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* referendum_id, unsigned long referendum_id_len,
    const char* proposal_id_hex, unsigned long proposal_id_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_governance_finalize_referendum_signed_transaction_alg(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* referendum_id, unsigned long referendum_id_len,
    const char* proposal_id_hex, unsigned long proposal_id_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t algorithm,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_governance_persist_council_signed_transaction(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    uint64_t epoch,
    uint32_t candidates_count,
    uint8_t derived_by,
    const uint8_t* members_json, unsigned long members_json_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_governance_persist_council_signed_transaction_alg(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    uint64_t epoch,
    uint32_t candidates_count,
    uint8_t derived_by,
    const uint8_t* members_json, unsigned long members_json_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t algorithm,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_mint_signed_transaction(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    uint32_t nonce,
    uint8_t nonce_present,
    const char* asset_definition, unsigned long asset_definition_len,
    const char* quantity, unsigned long quantity_len,
    const char* destination, unsigned long destination_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_mint_signed_transaction_alg(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    uint32_t nonce,
    uint8_t nonce_present,
    const char* asset_definition, unsigned long asset_definition_len,
    const char* quantity, unsigned long quantity_len,
    const char* destination, unsigned long destination_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t algorithm,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_burn_signed_transaction(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    uint32_t nonce,
    uint8_t nonce_present,
    const char* asset_definition, unsigned long asset_definition_len,
    const char* quantity, unsigned long quantity_len,
    const char* destination, unsigned long destination_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_burn_signed_transaction_alg(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    uint32_t nonce,
    uint8_t nonce_present,
    const char* asset_definition, unsigned long asset_definition_len,
    const char* quantity, unsigned long quantity_len,
    const char* destination, unsigned long destination_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t algorithm,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_decode_signed_transaction_json(
    const uint8_t* signed_bytes, unsigned long signed_len,
    uint8_t** out_json_ptr, unsigned long* out_json_len);

int32_t connect_norito_decode_asset_id_json(
    const char* asset_literal, unsigned long asset_len,
    uint8_t** out_json_ptr, unsigned long* out_json_len);

int32_t connect_norito_decode_transaction_receipt_json(
    const uint8_t* receipt_bytes, unsigned long receipt_len,
    uint8_t** out_json_ptr, unsigned long* out_json_len);

// ---------------- Acceleration configuration ----------------
typedef struct {
    uint8_t enable_simd;
    uint8_t enable_metal;
    uint8_t enable_cuda;
    uint64_t max_gpus;
    uint8_t max_gpus_present;
    uint64_t merkle_min_leaves_gpu;
    uint8_t merkle_min_leaves_gpu_present;
    uint64_t merkle_min_leaves_metal;
    uint8_t merkle_min_leaves_metal_present;
    uint64_t merkle_min_leaves_cuda;
    uint8_t merkle_min_leaves_cuda_present;
    uint64_t prefer_cpu_sha2_max_leaves_aarch64;
    uint8_t prefer_cpu_sha2_max_leaves_aarch64_present;
    uint64_t prefer_cpu_sha2_max_leaves_x86;
    uint8_t prefer_cpu_sha2_max_leaves_x86_present;
} connect_norito_acceleration_config;

void connect_norito_set_acceleration_config(const connect_norito_acceleration_config* cfg);
int32_t connect_norito_get_acceleration_config(connect_norito_acceleration_config* out_cfg);

typedef struct {
    uint8_t supported;
    uint8_t configured;
    uint8_t available;
    uint8_t parity_ok;
    // Optional UTF-8 error/disable message owned by the bridge.
    // Call `connect_norito_free(last_error_ptr)` after copying the bytes.
    uint8_t* last_error_ptr;
    unsigned long last_error_len;
} connect_norito_acceleration_backend_status;

typedef struct {
    connect_norito_acceleration_config config;
    connect_norito_acceleration_backend_status simd;
    connect_norito_acceleration_backend_status metal;
    connect_norito_acceleration_backend_status cuda;
} connect_norito_acceleration_state;

int32_t connect_norito_get_acceleration_state(connect_norito_acceleration_state* out_state);

#ifdef __cplusplus
} // extern "C"
#endif

#endif // CONNECT_NORITO_BRIDGE_H
