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
#define CONNECT_NORITO_ERR_KAGEMUSHA_PROVE -311
#define CONNECT_NORITO_ERR_KAGEMUSHA_RECURSIVE_SPEND_V2_UNAVAILABLE -314
#define CONNECT_NORITO_ERR_KAGEMUSHA_RECURSIVE_SPEND_V2_ARTIFACT -315
#define CONNECT_NORITO_ERR_SORAFS_REFERENCE -114
#define CONNECT_NORITO_ERR_DETACHED_TRANSACTION_SCAFFOLD -501
#define CONNECT_NORITO_ERR_DETACHED_TRANSACTION_SIGNATURE -502
#define CONNECT_NORITO_ERR_CANONICAL_JSON -503

#define CONNECT_NORITO_SORAFS_REFERENCE_ORDERBOOK_KIND_ORDER_REQUEST 1
#define CONNECT_NORITO_SORAFS_REFERENCE_ORDERBOOK_KIND_ORDER_CANCEL 2
#define CONNECT_NORITO_SORAFS_REFERENCE_ORDERBOOK_KIND_TRADE_EVENT 3
#define CONNECT_NORITO_SORAFS_REFERENCE_ORDERBOOK_KIND_SETTLEMENT_CHANNEL 4
#define CONNECT_NORITO_SORAFS_REFERENCE_ORDERBOOK_KIND_SETTLEMENT_RECEIPT 5
#define CONNECT_NORITO_SORAFS_REFERENCE_ORDERBOOK_KIND_RUNTIME_SNAPSHOT 6
#define CONNECT_NORITO_SORAFS_REFERENCE_HEDGING_KIND_PRICE_FEED 1
#define CONNECT_NORITO_SORAFS_REFERENCE_HEDGING_KIND_REFERENCE_PRICE_DECISION 2
#define CONNECT_NORITO_SORAFS_REFERENCE_HEDGING_KIND_BILLING_LINE_ITEM 3
#define CONNECT_NORITO_SORAFS_REFERENCE_HEDGING_KIND_BILLING_STATEMENT 4
#define CONNECT_NORITO_SORAFS_ORDERBOOK_SIDE_BID 1
#define CONNECT_NORITO_SORAFS_ORDERBOOK_SIDE_ASK 2
#define CONNECT_NORITO_SORAFS_ORDERBOOK_TIER_HOT 1
#define CONNECT_NORITO_SORAFS_ORDERBOOK_TIER_WARM 2
#define CONNECT_NORITO_SORAFS_ORDERBOOK_TIER_ARCHIVE 3
#define CONNECT_NORITO_SORAFS_ORDERBOOK_CANCEL_REASON_OWNER_REQUESTED 1
#define CONNECT_NORITO_SORAFS_ORDERBOOK_CANCEL_REASON_EXPIRED 2
#define CONNECT_NORITO_SORAFS_ORDERBOOK_CANCEL_REASON_GOVERNANCE 3
#define CONNECT_NORITO_SORAFS_ORDERBOOK_CANCEL_REASON_REPLACED 4
#define CONNECT_NORITO_SORAFS_ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1 256
#define CONNECT_NORITO_SORAFS_REFERENCE_PDP_KIND_COMMITMENT 1
#define CONNECT_NORITO_SORAFS_REFERENCE_PDP_KIND_CHALLENGE 2
#define CONNECT_NORITO_SORAFS_REFERENCE_PDP_KIND_PROOF 3

// ---------------- Bridge ABI ----------------
uint32_t connect_norito_bridge_abi_version(void);

// ---------------- Detached transaction verification ----------------

// Accepts only exact canonical versioned SignedTransaction bytes with a
// single-key Ed25519 authority, one primary signature slot, no nonce, no proof
// attachments, no multisig bundle, and exactly one supported executable:
// ContractCall or one numeric asset Transfer instruction. Returns compact,
// key-sorted JSON using schema iroha.detached_transaction_scaffold.v1.
int32_t connect_norito_detached_transaction_scaffold_inspect_v1(
    const uint8_t* tx,
    unsigned long tx_len,
    uint8_t** out_json,
    unsigned long* out_json_len);

// Re-validates the exact scaffold, binds the canonical 32-byte Ed25519 public
// key to its authority, admits and verifies the exact 64-byte signature over
// the payload signing hash, and returns canonical versioned signed transaction
// bytes plus iroha.detached_transaction_finalization.v1 JSON. All outputs are
// cleared on failure and must be released with connect_norito_free on success.
int32_t connect_norito_detached_transaction_scaffold_finalize_ed25519_v1(
    const uint8_t* tx,
    unsigned long tx_len,
    const uint8_t* public_key,
    unsigned long public_key_len,
    const uint8_t* signature,
    unsigned long signature_len,
    uint8_t** out_signed_tx,
    unsigned long* out_signed_tx_len,
    uint8_t** out_json,
    unsigned long* out_json_len);

// Strictly parses one complete JSON value (duplicates and trailing input are
// rejected), returns compact key-sorted Norito JSON, and writes its 32-byte
// BLAKE3 digest. A zero-length input intentionally maps to empty canonical
// bytes and BLAKE3(empty). out_hash_len must be exactly 32.
int32_t connect_norito_canonical_json_blake3_v1(
    const uint8_t* json,
    unsigned long json_len,
    uint8_t** out_canonical_json,
    unsigned long* out_canonical_json_len,
    uint8_t* out_hash,
    unsigned long out_hash_len);

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

// ---------------- Kagemusha recursive spend ABI 19 / artifact V3 ----------------
// Returns canonical Norito `KagemushaRecursiveSpendNativeCapabilitiesV1`.
// Callers must require `proof_backend_available`; symbol presence alone is not
// a production-readiness signal.
int32_t connect_norito_kagemusha_recursive_spend_capabilities_v1(
    uint8_t** out_capabilities_ptr,
    unsigned long* out_capabilities_len);

// Verifies canonical Norito `KagemushaTopUpFinalityProofV2` against the
// complete canonical `KagemushaRecursiveSpendTopUpAnchorV2` and a canonical,
// pre-fetched `KagemushaTopUpFinalityRosterArtifactV2`. The canonical V3
// manifest and its exact nonzero SHA-256 are passed directly; native code
// selects the roster descriptor from that typed manifest rather than trusting
// a parallel JSON projection or generation label. Returns 0 only after the
// manifest and roster digests, full anchor bindings, Commit-QC aggregate, and
// exact anchor path all verify. Recursive init performs this same verification
// inside its native boundary. This standalone symbol remains unavailable until
// the authenticated release-envelope trust root is wired.
int32_t connect_norito_kagemusha_topup_finality_verify_v2(
    const uint8_t* proof_norito_ptr,
    unsigned long proof_norito_len,
    const uint8_t* roster_norito_ptr,
    unsigned long roster_norito_len,
    const uint8_t* anchor_norito_ptr,
    unsigned long anchor_norito_len,
    const uint8_t* manifest_norito_ptr,
    unsigned long manifest_norito_len,
    const uint8_t* expected_manifest_sha256_ptr,
    unsigned long expected_manifest_sha256_len);

// Streams one complete published KRV3KEY package. Begin pins the canonical
// Norito manifest to a trusted SHA-256 and selects exactly one content-
// addressed artifact from it. Finalize re-parses and re-hashes the actual open
// file descriptor. A finalized handle does not authorize proving until all
// six roles are installed atomically and native capabilities also report the
// audited proof backend available.
int32_t connect_norito_kagemusha_recursive_spend_artifact_begin_v3(
    const uint8_t* manifest_norito_ptr,
    unsigned long manifest_norito_len,
    const uint8_t* expected_manifest_sha256_ptr,
    unsigned long expected_manifest_sha256_len,
    const uint8_t* expected_artifact_sha256_ptr,
    unsigned long expected_artifact_sha256_len,
    uint64_t* out_handle);
int32_t connect_norito_kagemusha_recursive_spend_artifact_write_v3(
    uint64_t handle,
    const uint8_t* chunk_ptr,
    unsigned long chunk_len);
int32_t connect_norito_kagemusha_recursive_spend_artifact_finalize_v3(uint64_t handle);
int32_t connect_norito_kagemusha_recursive_spend_artifact_cancel_v3(uint64_t handle);

// Installs exactly six finalized handles as one manifest-bound generation.
// Caller order is ignored; native code resolves and retains manifest order.
// Success consumes every handle atomically. Failure consumes none and leaves
// the previously installed generation unchanged.
int32_t connect_norito_kagemusha_recursive_spend_artifact_set_install_v3(
    const uint8_t* manifest_norito_ptr,
    unsigned long manifest_norito_len,
    const uint8_t* expected_manifest_sha256_ptr,
    unsigned long expected_manifest_sha256_len,
    const uint64_t* handles_ptr,
    unsigned long handles_len);
int32_t connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v3(
    const uint8_t* manifest_norito_ptr,
    unsigned long manifest_norito_len,
    const uint8_t* expected_manifest_sha256_ptr,
    unsigned long expected_manifest_sha256_len,
    uint8_t* out_installed);
// The digest guard prevents a stale owner from uninstalling a newer release.
int32_t connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v3(
    const uint8_t* expected_manifest_sha256_ptr,
    unsigned long expected_manifest_sha256_len);

// ---------------- Kagemusha first-release protocol ----------------

// Receiver request signing and sender verification. Signing-byte and digest
// outputs are raw byte strings (the digest is exactly 32 bytes); request inputs
// and outputs are canonical Norito archives.
int32_t connect_norito_kagemusha_receiver_key_reference_v2(
    const uint8_t* public_key_ptr,
    unsigned long public_key_len,
    uint8_t** out_reference_ptr,
    unsigned long* out_reference_len);

// Input is canonical `KagemushaRecipientOutputDerivationRequestV2`; the
// receiver note opening is a canonical local-only
// `connect_norito_bridge::KagemushaNoteOpeningV2` archive.
// Output is canonical `KagemushaRecipientOutputDerivationResultV2` and never
// contains the spend key or diversifier.
int32_t connect_norito_kagemusha_recipient_output_derive_v2(
    const uint8_t* request_norito_ptr,
    unsigned long request_norito_len,
    const uint8_t* receiver_note_opening_ptr,
    unsigned long receiver_note_opening_len,
    uint8_t** out_result_ptr,
    unsigned long* out_result_len);

// Input is canonical `KagemushaRecursiveSpendSplitIntentBuildRequestV2`.
// Parent provenance is derived exclusively from the embedded opaque bundles;
// output is canonical `KagemushaRecursiveSpendSplitIntentV2`.
int32_t connect_norito_kagemusha_recursive_spend_build_split_intent_v2(
    const uint8_t* request_norito_ptr,
    unsigned long request_norito_len,
    uint8_t** out_intent_ptr,
    unsigned long* out_intent_len);

int32_t connect_norito_kagemusha_recipient_payment_request_signing_bytes_v2(
    const uint8_t* payload_norito_ptr,
    unsigned long payload_norito_len,
    uint8_t** out_signing_bytes_ptr,
    unsigned long* out_signing_bytes_len);

int32_t connect_norito_kagemusha_recipient_payment_request_create_v2(
    const uint8_t* payload_norito_ptr,
    unsigned long payload_norito_len,
    const uint8_t* signature_ptr,
    unsigned long signature_len,
    uint8_t** out_request_ptr,
    unsigned long* out_request_len);

int32_t connect_norito_kagemusha_recipient_payment_request_verify_v2(
    const uint8_t* request_norito_ptr,
    unsigned long request_norito_len,
    uint64_t verified_at_ms,
    uint8_t** out_digest_ptr,
    unsigned long* out_digest_len);

// Authorization signing uses a canonical template with a disposable non-empty
// signature marker. The create call replaces that marker and verifies the real
// account signature before returning a protocol archive.
int32_t connect_norito_kagemusha_request_authorization_signing_bytes_v2(
    const uint8_t* template_norito_ptr,
    unsigned long template_norito_len,
    uint8_t** out_signing_bytes_ptr,
    unsigned long* out_signing_bytes_len);

int32_t connect_norito_kagemusha_request_authorization_create_v2(
    const uint8_t* template_norito_ptr,
    unsigned long template_norito_len,
    const uint8_t* signature_ptr,
    unsigned long signature_len,
    uint8_t** out_authorization_ptr,
    unsigned long* out_authorization_len);

// Durable receiver ACK lifecycle. Creation and verification bind the exact
// signed request and recipient-only peer payment; callers must additionally check the
// device key against their registered-device lineage policy.
int32_t connect_norito_kagemusha_receiver_acknowledgement_payload_v2(
    const uint8_t* request_norito_ptr,
    unsigned long request_norito_len,
    const uint8_t* peer_payment_norito_ptr,
    unsigned long peer_payment_norito_len,
    uint64_t accepted_at_ms,
    uint8_t** out_payload_ptr,
    unsigned long* out_payload_len);

int32_t connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2(
    const uint8_t* payload_norito_ptr,
    unsigned long payload_norito_len,
    uint8_t** out_signing_bytes_ptr,
    unsigned long* out_signing_bytes_len);

int32_t connect_norito_kagemusha_receiver_acknowledgement_create_v2(
    const uint8_t* payload_norito_ptr,
    unsigned long payload_norito_len,
    const uint8_t* signature_ptr,
    unsigned long signature_len,
    const uint8_t* request_norito_ptr,
    unsigned long request_norito_len,
    const uint8_t* peer_payment_norito_ptr,
    unsigned long peer_payment_norito_len,
    uint8_t** out_acknowledgement_ptr,
    unsigned long* out_acknowledgement_len);

int32_t connect_norito_kagemusha_receiver_acknowledgement_verify_v2(
    const uint8_t* acknowledgement_norito_ptr,
    unsigned long acknowledgement_norito_len,
    const uint8_t* request_norito_ptr,
    unsigned long request_norito_len,
    const uint8_t* peer_payment_norito_ptr,
    unsigned long peer_payment_norito_len,
    uint8_t** out_result_ptr,
    unsigned long* out_result_len);

// Recipient-only peer transport. The projection validates the split result
// and deliberately omits sender change. Validation returns the canonical
// payment archive for typed SDK decoding.
int32_t connect_norito_kagemusha_recursive_spend_peer_payment_from_split_v2(
    const uint8_t* split_result_norito_ptr,
    unsigned long split_result_norito_len,
    uint8_t** out_payment_ptr,
    unsigned long* out_payment_len);

int32_t connect_norito_kagemusha_recursive_spend_peer_payment_validate_v2(
    const uint8_t* payment_norito_ptr,
    unsigned long payment_norito_len,
    uint8_t** out_payment_ptr,
    unsigned long* out_payment_len);

// Proof/accumulator internals remain opaque to the SDK; this helper returns the
// validated wallet-safe `KagemushaRecursiveSpendBundleSummaryV2` archive.
int32_t connect_norito_kagemusha_recursive_spend_bundle_summary_v2(
    const uint8_t* bundle_norito_ptr,
    unsigned long bundle_norito_len,
    uint8_t** out_summary_ptr,
    unsigned long* out_summary_len);

int32_t connect_norito_kagemusha_recursive_spend_init_v2(
    const uint8_t* request_norito_ptr,
    unsigned long request_norito_len,
    uint8_t** out_init_result_ptr,
    unsigned long* out_init_result_len);

// The init request embeds the canonical top-up anchor, compact finality proof,
// and roster artifact. Native code authenticates the installed manifest,
// content-addresses the exact canonical roster bytes, and verifies the proof
// and anchor before invoking the recursive prover.

// Builds a canonical unsigned top-up from a local-only secret witness and the
// authoritative next-zero path returned by POST /v1/zk/merkle-path. Secret
// material is zeroized by native code and never appears in the output archive.
int32_t connect_norito_kagemusha_topup_shield_build_unsigned_v2(
    const uint8_t* request_norito_ptr,
    unsigned long request_norito_len,
    uint8_t** out_unsigned_ptr,
    unsigned long* out_unsigned_len);

int32_t connect_norito_kagemusha_recursive_spend_topup_unsigned_payload_digest_v2(
    const uint8_t* unsigned_norito_ptr,
    unsigned long unsigned_norito_len,
    uint8_t** out_digest_ptr,
    unsigned long* out_digest_len);

int32_t connect_norito_kagemusha_recursive_spend_topup_finalize_request_v2(
    const uint8_t* unsigned_norito_ptr,
    unsigned long unsigned_norito_len,
    const uint8_t* authorization_norito_ptr,
    unsigned long authorization_norito_len,
    uint8_t** out_request_ptr,
    unsigned long* out_request_len);

int32_t connect_norito_kagemusha_recursive_spend_topup_v2(
    const uint8_t* request_norito_ptr,
    unsigned long request_norito_len,
    uint8_t** out_instruction_ptr,
    unsigned long* out_instruction_len);

// Input is the native-only canonical
// `connect_norito_bridge::KagemushaRecursiveSpendAppendLocalRequestV2`:
// opaque parents plus local note openings, exact Merkle membership witnesses,
// optional sender-change opening, active transfer verifier binding, operation
// id, and block height. Secrets are zeroized before return. The entrypoint
// remains unavailable until recursive append can atomically return both the
// split result and proof-output-bound recipient/change membership witnesses;
// a bundle without those witnesses is not spendable cash.
int32_t connect_norito_kagemusha_recursive_spend_append_v2(
    const uint8_t* request_norito_ptr,
    unsigned long request_norito_len,
    const uint8_t* recipient_request_norito_ptr,
    unsigned long recipient_request_norito_len,
    uint64_t verified_at_ms,
    uint8_t** out_split_result_ptr,
    unsigned long* out_split_result_len);

int32_t connect_norito_kagemusha_recursive_spend_verify_v2(
    const uint8_t* request_norito_ptr,
    unsigned long request_norito_len,
    uint8_t** out_result_ptr,
    unsigned long* out_result_len);

int32_t connect_norito_kagemusha_recursive_spend_redeem_unsigned_payload_digest_v2(
    const uint8_t* unsigned_norito_ptr,
    unsigned long unsigned_norito_len,
    uint8_t** out_digest_ptr,
    unsigned long* out_digest_len);

// Input is the canonical `KagemushaRecursiveSpendRedeemBuildResultV2`
// returned by the native proof builder. Finalization preserves its optional
// offline change bundle and proof-bound membership witness atomically.
int32_t connect_norito_kagemusha_recursive_spend_redeem_finalize_request_v2(
    const uint8_t* build_result_norito_ptr,
    unsigned long build_result_norito_len,
    const uint8_t* authorization_norito_ptr,
    unsigned long authorization_norito_len,
    uint8_t** out_result_ptr,
    unsigned long* out_result_len);

// Input is the native-only canonical
// `connect_norito_bridge::KagemushaRecursiveSpendRedeemLocalRequestV2` with
// the owned opening, exact membership/dummy paths, exact scaled public amount,
// optional private change opening, and active unshield-v3 verifier binding.
// Native derives the unshield proof attachment and redemption intent; callers
// cannot supply either. This remains unavailable until partial redemption can
// atomically return the proof-bound offline-change membership witness.
int32_t connect_norito_kagemusha_recursive_spend_redeem_v2(
    const uint8_t* request_norito_ptr,
    unsigned long request_norito_len,
    uint8_t** out_build_result_ptr,
    unsigned long* out_build_result_len);

void connect_norito_free(uint8_t* ptr);

// ---------------- Privacy proof native FFI ----------------
// Output buffers are Norito V1 archives allocated by the bridge and must be
// released with `iroha_privacy_free_buffer`, which zeroizes privacy output
// memory before release.
int32_t iroha_privacy_capabilities_v1(
    uint8_t** out_ptr,
    unsigned long* out_len);

int32_t iroha_privacy_proof_request_v1(
    const uint8_t* algorithm_id_ptr,
    unsigned long algorithm_id_len,
    const uint8_t* entrypoint_ptr,
    unsigned long entrypoint_len,
    const uint8_t* vk_ref_ptr,
    unsigned long vk_ref_len,
    const uint8_t* public_inputs_ptr,
    unsigned long public_inputs_len,
    const uint8_t* witness_ptr,
    unsigned long witness_len,
    const uint8_t* proof_ptr,
    unsigned long proof_len,
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
int32_t connect_norito_sorafs_reference_validate_orderbook_json(
    uint32_t kind,
    const uint8_t* bytes_ptr,
    unsigned long bytes_len,
    const uint8_t* label_ptr,
    unsigned long label_len,
    uint64_t generated_at,
    uint8_t** out_json_ptr,
    unsigned long* out_json_len);

int32_t connect_norito_sorafs_reference_validate_pop_json(
    uint32_t kind,
    const uint8_t* bytes_ptr,
    unsigned long bytes_len,
    const uint8_t* label_ptr,
    unsigned long label_len,
    uint64_t generated_at,
    uint8_t** out_json_ptr,
    unsigned long* out_json_len);

int32_t connect_norito_sorafs_reference_validate_hedging_json(
    uint32_t kind,
    const uint8_t* bytes_ptr,
    unsigned long bytes_len,
    const uint8_t* label_ptr,
    unsigned long label_len,
    uint64_t generated_at,
    uint8_t** out_json_ptr,
    unsigned long* out_json_len);

int32_t connect_norito_sorafs_reference_sign_orderbook_payload(
    uint32_t kind,
    const uint8_t* bytes_ptr,
    unsigned long bytes_len,
    const uint8_t* private_key_ptr,
    unsigned long private_key_len,
    uint8_t** out_signed_ptr,
    unsigned long* out_signed_len);

int32_t connect_norito_sorafs_reference_derive_orderbook_order_id(
    const uint8_t* owner_account_ptr,
    unsigned long owner_account_len,
    uint64_t nonce,
    uint8_t* out_order_id_ptr,
    unsigned long out_order_id_len);

int32_t connect_norito_sorafs_reference_build_signed_orderbook_order_request(
    const uint8_t* order_id_ptr,
    unsigned long order_id_len,
    uint32_t side,
    uint32_t tier,
    const uint8_t* price_per_gib_micro_xor_ptr,
    unsigned long price_per_gib_micro_xor_len,
    uint64_t quantity_gib,
    uint64_t remaining_gib,
    const uint8_t* owner_account_ptr,
    unsigned long owner_account_len,
    uint64_t expiry_unix,
    uint64_t nonce,
    uint32_t maker_fee_bps,
    uint32_t taker_fee_bps,
    const uint8_t* private_key_ptr,
    unsigned long private_key_len,
    uint8_t** out_signed_ptr,
    unsigned long* out_signed_len);

int32_t connect_norito_sorafs_reference_build_signed_orderbook_order_cancel(
    const uint8_t* order_id_ptr,
    unsigned long order_id_len,
    const uint8_t* owner_account_ptr,
    unsigned long owner_account_len,
    uint32_t reason,
    uint64_t nonce,
    const uint8_t* private_key_ptr,
    unsigned long private_key_len,
    uint8_t** out_signed_ptr,
    unsigned long* out_signed_len);

int32_t connect_norito_sorafs_reference_build_signed_orderbook_settlement_receipt(
    const uint8_t* receipt_id_ptr,
    unsigned long receipt_id_len,
    const uint8_t* channel_id_ptr,
    unsigned long channel_id_len,
    const uint8_t* trade_id_ptr,
    unsigned long trade_id_len,
    uint64_t range_start,
    uint64_t range_end,
    const uint8_t* chunk_hash_ptr,
    unsigned long chunk_hash_len,
    uint64_t bytes_delivered,
    const uint8_t* xor_debited_micro_xor_ptr,
    unsigned long xor_debited_micro_xor_len,
    const uint8_t* provider_credit_micro_xor_ptr,
    unsigned long provider_credit_micro_xor_len,
    const uint8_t* fee_amount_micro_xor_ptr,
    unsigned long fee_amount_micro_xor_len,
    uint64_t issued_at_unix,
    const uint8_t* private_key_ptr,
    unsigned long private_key_len,
    uint8_t** out_signed_ptr,
    unsigned long* out_signed_len);

int32_t connect_norito_sorafs_reference_validate_pdp_payload_json(
    uint32_t kind,
    const uint8_t* bytes_ptr,
    unsigned long bytes_len,
    const uint8_t* label_ptr,
    unsigned long label_len,
    uint64_t generated_at,
    uint8_t** out_json_ptr,
    unsigned long* out_json_len);

int32_t connect_norito_sorafs_reference_validate_pdp_commitment_challenge_json(
    const uint8_t* commitment_ptr,
    unsigned long commitment_len,
    const uint8_t* commitment_label_ptr,
    unsigned long commitment_label_len,
    const uint8_t* challenge_ptr,
    unsigned long challenge_len,
    const uint8_t* challenge_label_ptr,
    unsigned long challenge_label_len,
    uint64_t generated_at,
    uint8_t** out_json_ptr,
    unsigned long* out_json_len);

int32_t connect_norito_sorafs_reference_validate_pdp_challenge_proof_json(
    const uint8_t* challenge_ptr,
    unsigned long challenge_len,
    const uint8_t* challenge_label_ptr,
    unsigned long challenge_label_len,
    const uint8_t* proof_ptr,
    unsigned long proof_len,
    const uint8_t* proof_label_ptr,
    unsigned long proof_label_len,
    uint64_t generated_at,
    uint8_t** out_json_ptr,
    unsigned long* out_json_len);

int32_t connect_norito_sorafs_reference_validate_pdp_bundle_json(
    const uint8_t* commitment_ptr,
    unsigned long commitment_len,
    const uint8_t* commitment_label_ptr,
    unsigned long commitment_label_len,
    const uint8_t* challenge_ptr,
    unsigned long challenge_len,
    const uint8_t* challenge_label_ptr,
    unsigned long challenge_label_len,
    const uint8_t* proof_ptr,
    unsigned long proof_len,
    const uint8_t* proof_label_ptr,
    unsigned long proof_label_len,
    uint64_t generated_at,
    uint8_t** out_json_ptr,
    unsigned long* out_json_len);

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

int32_t connect_norito_encode_transfer_instruction_box(
    const char* authority, unsigned long authority_len,
    const char* asset_definition, unsigned long asset_definition_len,
    const char* quantity, unsigned long quantity_len,
    const char* destination, unsigned long destination_len,
    uint8_t** out_instruction_ptr, unsigned long* out_instruction_len);

int32_t connect_norito_encode_validation_fee_transfer_signed_transaction(
    const char* chain_id, unsigned long chain_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    uint32_t nonce,
    uint8_t nonce_present,
    const char* principal_asset_definition, unsigned long principal_asset_definition_len,
    const char* principal_quantity, unsigned long principal_quantity_len,
    const char* destination, unsigned long destination_len,
    const char* fee_asset_definition, unsigned long fee_asset_definition_len,
    const char* fee_quantity, unsigned long fee_quantity_len,
    const char* treasury, unsigned long treasury_len,
    uint64_t policy_version,
    const char* policy_hash, unsigned long policy_hash_len,
    uint64_t fee_instruction_index,
    const char* fee_sponsor, unsigned long fee_sponsor_len,
    const char* memo, unsigned long memo_len,
    const char* metadata_json, unsigned long metadata_json_len,
    const uint8_t* private_key, unsigned long private_key_len,
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
