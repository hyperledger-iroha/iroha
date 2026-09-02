// NoritoBridge C FFI header
// Place this header into the XCFramework at: NoritoBridge.xcframework/**/Headers/connect_norito_bridge.h
// And include a modulemap at: NoritoBridge.xcframework/**/Modules/module.modulemap
//
// module.modulemap example:
//   module NoritoBridge { header "connect_norito_bridge.h" export * }

#ifndef CONNECT_NORITO_BRIDGE_H
#define CONNECT_NORITO_BRIDGE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define CONNECT_NORITO_BRIDGE_ABI_VERSION 23

#define CONNECT_NORITO_ERR_ACCOUNT_ADDRESS -200
#define CONNECT_NORITO_ERR_UNSUPPORTED_ALGORITHM -21
#define CONNECT_NORITO_ERR_OFFLINE_CASH_V1 -311
#define CONNECT_NORITO_ERR_OFFLINE_CASH_DEVICE_UNAVAILABLE_V1 -312
#define CONNECT_NORITO_ERR_SORAFS_REFERENCE -114
#define CONNECT_NORITO_ERR_DETACHED_TRANSACTION_SCAFFOLD -501
#define CONNECT_NORITO_ERR_DETACHED_TRANSACTION_SIGNATURE -502
#define CONNECT_NORITO_ERR_CANONICAL_JSON -503
#define CONNECT_NORITO_ERR_VALIDATION_FEE_POLICY_PROOF -504
#define CONNECT_NORITO_ERR_PARLIAMENT_TIMED_OVN -505
#define CONNECT_NORITO_ERR_VALIDATION_FEE_HIJIRI_QUOTE -506
#define CONNECT_NORITO_ERR_PRIVATE_SETTLEMENT_RESPONSE -507
#define CONNECT_NORITO_ERR_CONNECT_IDENTITY -410
#define CONNECT_NORITO_ERR_CONNECT_APPROVAL -411

#define CONNECT_NORITO_VALIDATION_FEE_HIJIRI_QUOTE_VERSION_V1 1
#define CONNECT_NORITO_VALIDATION_FEE_HIJIRI_QUOTE_MAX_TRANSFERS_V1 100000
#define CONNECT_NORITO_VALIDATION_FEE_HIJIRI_QUOTE_MAX_REQUEST_BYTES_V1 4096
#define CONNECT_NORITO_VALIDATION_FEE_HIJIRI_QUOTE_MAX_RESPONSE_BYTES_V1 65536

#define CONNECT_NORITO_PRIVATE_SETTLEMENT_REQUEST_MAX_BYTES_V1 1048576
#define CONNECT_NORITO_PRIVATE_SETTLEMENT_RESPONSE_MAX_BYTES_V1 33554432

#define CONNECT_NORITO_PARLIAMENT_TIMED_OVN_SEED_BYTES_V1 32
#define CONNECT_NORITO_PARLIAMENT_TIMED_OVN_TRUST_ANCHOR_BYTES_V1 32
#define CONNECT_NORITO_PARLIAMENT_TIMED_OVN_CASTING_PROOF_MAX_BYTES_V1 8388608
#define CONNECT_NORITO_PARLIAMENT_TIMED_OVN_CASTING_PROOF_PAGE_RESULT_BYTES_V1 41

#define CONNECT_NORITO_SORAFS_REFERENCE_ORDERBOOK_KIND_ORDER_REQUEST 1
#define CONNECT_NORITO_SORAFS_REFERENCE_ORDERBOOK_KIND_ORDER_CANCEL 2
#define CONNECT_NORITO_SORAFS_REFERENCE_ORDERBOOK_KIND_TRADE_EVENT 3
#define CONNECT_NORITO_SORAFS_REFERENCE_ORDERBOOK_KIND_SETTLEMENT_CHANNEL 4
#define CONNECT_NORITO_SORAFS_REFERENCE_ORDERBOOK_KIND_SETTLEMENT_RECEIPT 5
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
#define CONNECT_NORITO_SORAFS_REFERENCE_BUNDLE_KIND_PROVIDER_ADVERT 1
#define CONNECT_NORITO_SORAFS_REFERENCE_BUNDLE_KIND_PROVIDER_ADMISSION_ENVELOPE 2
#define CONNECT_NORITO_SORAFS_REFERENCE_BUNDLE_KIND_REPLICATION_ORDER 3
#define CONNECT_NORITO_SORAFS_REFERENCE_BUNDLE_KIND_POR_CHALLENGE 4
#define CONNECT_NORITO_SORAFS_REFERENCE_BUNDLE_KIND_POR_PROOF 5
#define CONNECT_NORITO_SORAFS_REFERENCE_BUNDLE_KIND_POTR_RECEIPT 6
#define CONNECT_NORITO_SORAFS_REFERENCE_BUNDLE_KIND_REPAIR_EVIDENCE 7
#define CONNECT_NORITO_SORAFS_REFERENCE_BUNDLE_KIND_REPAIR_REPORT 8
#define CONNECT_NORITO_SORAFS_REFERENCE_BUNDLE_KIND_REPAIR_TASK_RECORD 9
#define CONNECT_NORITO_SORAFS_REFERENCE_BUNDLE_KIND_REPAIR_SLASH_PROPOSAL 10
#define CONNECT_NORITO_SORAFS_REFERENCE_BUNDLE_KIND_REPAIR_TASK_EVENT 11
#define CONNECT_NORITO_SORAFS_REFERENCE_BUNDLE_KIND_ORDERBOOK_ORDER_REQUEST 12
#define CONNECT_NORITO_SORAFS_REFERENCE_BUNDLE_KIND_ORDERBOOK_ORDER_CANCEL 13
#define CONNECT_NORITO_SORAFS_REFERENCE_BUNDLE_KIND_ORDERBOOK_TRADE_EVENT 14
#define CONNECT_NORITO_SORAFS_REFERENCE_BUNDLE_KIND_ORDERBOOK_SETTLEMENT_CHANNEL 15
#define CONNECT_NORITO_SORAFS_REFERENCE_BUNDLE_KIND_ORDERBOOK_SETTLEMENT_RECEIPT 16
#define CONNECT_NORITO_SORAFS_REFERENCE_BUNDLE_KIND_PDP_COMMITMENT 17
#define CONNECT_NORITO_SORAFS_REFERENCE_BUNDLE_KIND_PDP_CHALLENGE 18
#define CONNECT_NORITO_SORAFS_REFERENCE_BUNDLE_KIND_PDP_PROOF 19
#define CONNECT_NORITO_SORAFS_REFERENCE_GOVERNANCE_DAG_MAX_BLOCKS_V1 64
#define CONNECT_NORITO_SORAFS_REFERENCE_GOVERNANCE_DAG_CID_BYTES_V1 32
#define CONNECT_NORITO_SORAFS_REFERENCE_MAX_INPUT_BYTES_V1 67108864
#define CONNECT_NORITO_SORAFS_REFERENCE_MAX_LABEL_BYTES_V1 1024
#define CONNECT_NORITO_SORAFS_REFERENCE_BUNDLE_MAX_PAYLOADS_V1 64
#define CONNECT_NORITO_SORAFS_REFERENCE_BUNDLE_MAX_TOTAL_BYTES_V1 67108864

typedef struct ConnectNoritoSorafsReferenceInput {
  const uint8_t* bytes_ptr;
  size_t bytes_len;
  const uint8_t* label_ptr;
  size_t label_len;
} ConnectNoritoSorafsReferenceInput;

typedef struct ConnectNoritoSorafsReferenceBundlePayload {
  uint32_t kind;
  const uint8_t* bytes_ptr;
  size_t bytes_len;
  const uint8_t* label_ptr;
  size_t label_len;
} ConnectNoritoSorafsReferenceBundlePayload;

// ---------------- Bridge ABI ----------------
uint32_t connect_norito_bridge_abi_version(void);

// Releases any bridge-owned byte buffer returned through an out pointer.
void connect_norito_free(uint8_t *ptr);

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

// Parses the exact typed sponsored-onboarding plan-body JSON and returns its
// bare canonical Norito V1 bytes. The output is cleared on failure and must be
// released with connect_norito_free on success.
int32_t connect_norito_encode_account_onboarding_plan_body_v1(
    const uint8_t* json,
    unsigned long json_len,
    uint8_t** out_body,
    unsigned long* out_body_len);

// Decodes one alias instruction through the Rust instruction registry under
// its exact stable wire ID, then returns the complete canonical Norito frame
// and a typed JSON envelope. Both outputs are cleared on failure and must be
// released with connect_norito_free on success.
int32_t connect_norito_alias_instruction_round_trip_v1(
    const uint8_t* wire_id,
    unsigned long wire_id_len,
    const uint8_t* framed_payload,
    unsigned long framed_payload_len,
    uint8_t** out_framed_payload,
    unsigned long* out_framed_payload_len,
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

// Encodes the canonical Norito V1 request body for
// POST /v1/validation-fee/policy/current/proof. The context id is validated
// for symmetry with the verifier but is not serialized by the frozen request.
int32_t connect_norito_validation_fee_current_policy_proof_request_v1(
    uint64_t trusted_checkpoint_height,
    const uint8_t* trusted_checkpoint_context_id,
    unsigned long trusted_checkpoint_context_id_len,
    uint8_t** out_request,
    unsigned long* out_request_len);

// Verifies one canonical Norito proof page against finality, its synthetic
// ordinary-write witness, the complete registry, and all immutable deployment
// bindings. On success it returns canonical JSON using schema
// iroha.validation_fee.verified_policy_projection.v1. The output is cleared on
// failure and must be released with connect_norito_free on success.
int32_t connect_norito_validation_fee_current_policy_proof_verify_v1(
    const uint8_t* proof_norito,
    unsigned long proof_norito_len,
    const uint8_t* network_id,
    unsigned long network_id_len,
    const uint8_t* policy_chain_genesis_hash,
    unsigned long policy_chain_genesis_hash_len,
    uint64_t trusted_checkpoint_height,
    const uint8_t* trusted_checkpoint_context_id,
    unsigned long trusted_checkpoint_context_id_len,
    uint8_t** out_projection_json,
    unsigned long* out_projection_json_len);

// Encodes the exact canonical Norito V1 request body for
// POST /v1/validation-fee/hijiri/quote. The account id must be a canonical
// I105 literal and qualifying_transfer_count must be in 1..100000. The output
// is cleared on failure and must be released with connect_norito_free on
// success.
int32_t connect_norito_validation_fee_hijiri_quote_request_v1(
    const uint8_t* account_id_utf8,
    unsigned long account_id_len,
    uint32_t qualifying_transfer_count,
    uint8_t** out_request,
    unsigned long* out_request_len);

// Canonically decodes and re-encodes both native-Norito archives, validates
// the response's full Hijiri arithmetic/coherence, and binds its echoed
// account, transfer count, and checked successor height to the exact request.
// The response is bounded to 64 KiB and the request to 4 KiB. On success this
// returns typed canonical Norito JSON using schema
// iroha.torii.v1.validation_fee.hijiri_quote.response. The output is cleared
// on failure and must be released with connect_norito_free on success.
int32_t connect_norito_validation_fee_hijiri_quote_response_verify_v1(
    const uint8_t* response_norito,
    unsigned long response_norito_len,
    const uint8_t* request_norito,
    unsigned long request_norito_len,
    uint8_t** out_projection_json,
    unsigned long* out_projection_json_len);

// Verifies the complete typed committee proof view, including all manifest,
// statement, delta, approval, availability, roster-PoP, and network bindings.
// expected_network_id and requested_payload_digest must each contain exactly
// 32 bytes. The response is bounded to 32 MiB and no restricted bytes are
// returned. Zero means success; failures use the single redacted -507 code.
int32_t connect_norito_private_settlement_committee_proof_response_verify_v1(
    const uint8_t* response_json,
    unsigned long response_json_len,
    const uint8_t* expected_network_id,
    unsigned long expected_network_id_len,
    const uint8_t* requested_payload_digest,
    unsigned long requested_payload_digest_len);

// Verifies one exact policy-bearing auditor capsule POST request and its
// response, including responder attestation, governed auditor signing-key
// membership, request-policy equality, and consensus/auditor key separation.
// The request is bounded to 1 MiB and the response to 32 MiB. No plaintext is
// decrypted or returned by this verifier.
int32_t connect_norito_private_settlement_auditor_capsule_response_verify_with_request_v1(
    const uint8_t* response_json,
    unsigned long response_json_len,
    const uint8_t* request_json,
    unsigned long request_json_len,
    const uint8_t* expected_network_id,
    unsigned long expected_network_id_len,
    const uint8_t* requested_payload_digest,
    unsigned long requested_payload_digest_len,
    const char* auditor_signing_key,
    unsigned long auditor_signing_key_len);

// Verifies the exact signed auditor approval request and its typed responder
// acknowledgement. The request is bounded to 1 MiB and the response to
// 32 MiB; no request, response, or restricted capsule bytes are returned.
int32_t connect_norito_private_settlement_audit_approval_response_verify_v1(
    const uint8_t* response_json,
    unsigned long response_json_len,
    const uint8_t* request_json,
    unsigned long request_json_len,
    const uint8_t* expected_network_id,
    unsigned long expected_network_id_len,
    const uint8_t* requested_payload_digest,
    unsigned long requested_payload_digest_len,
    const char* auditor_signing_key,
    unsigned long auditor_signing_key_len);

// ---------------- Parliament timed-OVN wallet operations ----------------

// Authenticates one bounded proof page against independently configured
// network/checkpoint/ballot anchors. The caller-owned output must contain
// exactly 41 bytes: big-endian u64 evaluated height, 32-byte evaluated context
// id, and canonical 0/1 more-available. Intermediate pages promote only the
// checkpoint; terminal pages also replay and bind the complete Core archive.
int32_t connect_norito_parliament_timed_ovn_verify_casting_proof_page_v1(
    const uint8_t* proof_response_norito,
    unsigned long proof_response_norito_len,
    const uint8_t* network_id,
    unsigned long network_id_len,
    uint64_t trusted_checkpoint_height,
    const uint8_t* trusted_checkpoint_context_id,
    unsigned long trusted_checkpoint_context_id_len,
    const uint8_t* expected_ballot_attempt_id,
    unsigned long expected_ballot_attempt_id_len,
    uint8_t* out_page_result,
    unsigned long out_page_result_len);

// Authenticates a terminal proof response against independently configured
// network/checkpoint/ballot anchors, then canonical-decodes and replay-validates
// its Core archive and rederives the authenticated compact binding.
int32_t connect_norito_parliament_timed_ovn_verify_casting_proof_v1(
    const uint8_t* proof_response_norito,
    unsigned long proof_response_norito_len,
    const uint8_t* network_id,
    unsigned long network_id_len,
    uint64_t trusted_checkpoint_height,
    const uint8_t* trusted_checkpoint_context_id,
    unsigned long trusted_checkpoint_context_id_len,
    const uint8_t* expected_ballot_attempt_id,
    unsigned long expected_ballot_attempt_id_len);

// Verifies the same proof and archive before reading the exact 32-byte
// caller-keystore seed. Only the public registration record is returned.
// The two output slots must be distinct and must not overlap input storage.
int32_t connect_norito_parliament_timed_ovn_registration_from_proof_v1(
    const uint8_t* proof_response_norito,
    unsigned long proof_response_norito_len,
    const uint8_t* network_id,
    unsigned long network_id_len,
    uint64_t trusted_checkpoint_height,
    const uint8_t* trusted_checkpoint_context_id,
    unsigned long trusted_checkpoint_context_id_len,
    const uint8_t* expected_ballot_attempt_id,
    unsigned long expected_ballot_attempt_id_len,
    const char* authority,
    unsigned long authority_len,
    const uint8_t* keystore_seed,
    unsigned long keystore_seed_len,
    uint8_t** out_registration,
    unsigned long* out_registration_len);

// Verifies the same proof and archive before reading the seed, reconstructs the
// exact committed registration, and returns a survivor- and release-bound
// masked ballot. choice is 0 (Aye), 1 (Nay), or 2 (Abstain).
// The two output slots must be distinct and must not overlap input storage.
int32_t connect_norito_parliament_timed_ovn_ballot_from_proof_v1(
    const uint8_t* proof_response_norito,
    unsigned long proof_response_norito_len,
    const uint8_t* network_id,
    unsigned long network_id_len,
    uint64_t trusted_checkpoint_height,
    const uint8_t* trusted_checkpoint_context_id,
    unsigned long trusted_checkpoint_context_id_len,
    const uint8_t* expected_ballot_attempt_id,
    unsigned long expected_ballot_attempt_id_len,
    const char* authority,
    unsigned long authority_len,
    const uint8_t* keystore_seed,
    unsigned long keystore_seed_len,
    uint8_t choice,
    uint8_t** out_ballot,
    unsigned long* out_ballot_len);

// ---------------- Chain discriminant helpers ----------------
// Thread-scoped overrides must be exited on the same thread and in LIFO order.
// enter returns zero on failure; exit returns zero on success and -1 on misuse.
uint64_t connect_norito_chain_discriminant_scope_enter(uint16_t discriminant);
int32_t connect_norito_chain_discriminant_scope_exit(uint64_t token);

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

// ---------------- Offline Cash V1 ----------------
// All raw and `oc1:` validators enforce their protocol byte limits before decode.
int32_t connect_norito_offline_cash_v1_payment_request_validate(
    const uint8_t* request, unsigned long request_len);
int32_t connect_norito_offline_cash_v1_acceptance_intent_authorization_validate(
    const uint8_t* request, unsigned long request_len,
    const uint8_t* authorization, unsigned long authorization_len);
int32_t connect_norito_offline_cash_v1_acceptance_ticket_validate(
    const uint8_t* request, unsigned long request_len,
    const uint8_t* authorization, unsigned long authorization_len,
    const uint8_t* ticket, unsigned long ticket_len);
int32_t connect_norito_offline_cash_v1_no_commit_closure_validate(
    const uint8_t* closure, unsigned long closure_len);
int32_t connect_norito_offline_cash_v1_payment_validate(
    const uint8_t* request, unsigned long request_len,
    const uint8_t* payment, unsigned long payment_len);
int32_t connect_norito_offline_cash_v1_acknowledgement_validate(
    const uint8_t* request, unsigned long request_len,
    const uint8_t* payment, unsigned long payment_len,
    const uint8_t* acknowledgement, unsigned long acknowledgement_len);
// Validates all five separately transported messages, including their aggregate
// byte cap and the standalone authorization/ticket binding embedded by payment.
int32_t connect_norito_offline_cash_v1_complete_exchange_validate(
    const uint8_t* request, unsigned long request_len,
    const uint8_t* authorization, unsigned long authorization_len,
    const uint8_t* ticket, unsigned long ticket_len,
    const uint8_t* payment, unsigned long payment_len,
    const uint8_t* acknowledgement, unsigned long acknowledgement_len);
int32_t connect_norito_offline_cash_v1_mint_authorization_validate(
    const uint8_t* authorization, unsigned long authorization_len);
int32_t connect_norito_offline_cash_v1_mint_credit_validate(
    const uint8_t* credit, unsigned long credit_len);
int32_t connect_norito_offline_cash_v1_mint_credit_against_authorization_validate(
    const uint8_t* authorization, unsigned long authorization_len,
    const uint8_t* credit, unsigned long credit_len);
int32_t connect_norito_offline_cash_v1_redemption_voucher_validate(
    const uint8_t* voucher, unsigned long voucher_len);

int32_t connect_norito_offline_cash_v1_payment_request_text_validate(
    const char* request, unsigned long request_len);
int32_t connect_norito_offline_cash_v1_acceptance_intent_authorization_text_validate(
    const char* request, unsigned long request_len,
    const char* authorization, unsigned long authorization_len);
int32_t connect_norito_offline_cash_v1_acceptance_ticket_text_validate(
    const char* request, unsigned long request_len,
    const char* authorization, unsigned long authorization_len,
    const char* ticket, unsigned long ticket_len);
int32_t connect_norito_offline_cash_v1_no_commit_closure_text_validate(
    const char* closure, unsigned long closure_len);
int32_t connect_norito_offline_cash_v1_payment_text_validate(
    const char* request, unsigned long request_len,
    const char* payment, unsigned long payment_len);
int32_t connect_norito_offline_cash_v1_acknowledgement_text_validate(
    const char* request, unsigned long request_len,
    const char* payment, unsigned long payment_len,
    const char* acknowledgement, unsigned long acknowledgement_len);
int32_t connect_norito_offline_cash_v1_complete_exchange_text_validate(
    const char* request, unsigned long request_len,
    const char* authorization, unsigned long authorization_len,
    const char* ticket, unsigned long ticket_len,
    const char* payment, unsigned long payment_len,
    const char* acknowledgement, unsigned long acknowledgement_len);
int32_t connect_norito_offline_cash_v1_mint_authorization_text_validate(
    const char* authorization, unsigned long authorization_len);
int32_t connect_norito_offline_cash_v1_mint_credit_text_validate(
    const char* credit, unsigned long credit_len);
int32_t connect_norito_offline_cash_v1_mint_credit_against_authorization_text_validate(
    const char* authorization, unsigned long authorization_len,
    const char* credit, unsigned long credit_len);
int32_t connect_norito_offline_cash_v1_redemption_voucher_text_validate(
    const char* voucher, unsigned long voucher_len);

// Closed lower-sixteen-bit capability mask shared with OfflineCashHardwareProfileV1.
#define CONNECT_NORITO_OFFLINE_CASH_DEVICE_REQUIRED_CAPABILITIES_V1 UINT32_C(0x0000FFFF)

typedef enum ConnectNoritoOfflineCashDeviceCapabilityV1 {
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_CAPABILITY_EXACT_NEXT_PREDECESSOR_CONSUMPTION_V1 = 1u << 0,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_CAPABILITY_ONE_USE_SUCCESSOR_AUTHORIZATION_V1 = 1u << 1,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_CAPABILITY_ROLLBACK_RESISTANT_COUNTER_AND_JOURNAL_V1 = 1u << 2,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_CAPABILITY_SEALED_TRANSITION_RECOVERY_V1 = 1u << 3,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_CAPABILITY_ONE_USE_ACCEPTANCE_TICKETS_V1 = 1u << 4,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_CAPABILITY_DURABLE_INBOX_RESERVATION_V1 = 1u << 5,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_CAPABILITY_AUTHENTICATED_INBOUND_STAGING_V1 = 1u << 6,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_CAPABILITY_AUTHORITATIVE_REPLAY_ROOT_RECOVERY_V1 = 1u << 7,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_CAPABILITY_SENDER_OUTBOX_RESERVATION_V1 = 1u << 8,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_CAPABILITY_AUTHENTICATED_DURABLE_RETRY_OUTBOX_V1 = 1u << 9,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_CAPABILITY_ATOMIC_VERIFIED_CANDIDATE_COMMIT_V1 = 1u << 10,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_CAPABILITY_RECOVERABLE_TERMINAL_COMMIT_CERTIFICATE_V1 = 1u << 11,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_CAPABILITY_TRUSTED_TIME_OR_LEASE_V1 = 1u << 12,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_CAPABILITY_OFFLINE_HARDWARE_EPOCH_ROTATION_V1 = 1u << 13,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_CAPABILITY_ROLLBACK_SAFE_COUNTER_ROLLOVER_V1 = 1u << 14,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_CAPABILITY_NO_SOFTWARE_FALLBACK_V1 = 1u << 15
} ConnectNoritoOfflineCashDeviceCapabilityV1;

// Values are encoded in the command frame's one-byte operation field; the enum itself is not
// passed as a C ABI argument.
typedef enum ConnectNoritoOfflineCashDeviceOperationV1 {
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_OPERATION_READ_ACTIVE_HARDWARE_CREDENTIAL_V1 = 1,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_OPERATION_PREPARE_ACCEPTANCE_INTENT_AUTHORIZATION_V1 = 2,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_OPERATION_RECOVER_ACCEPTANCE_INTENT_AUTHORIZATION_V1 = 3,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_OPERATION_VERIFY_AUTHORIZATION_RESERVE_INBOX_AND_ISSUE_ACCEPTANCE_TICKET_V1 = 4,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_OPERATION_RECOVER_ACCEPTANCE_TICKET_V1 = 5,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_OPERATION_STAGE_INBOUND_PAYMENT_V1 = 6,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_OPERATION_RECOVER_STAGED_INBOUND_PAYMENT_V1 = 7,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_OPERATION_RECOVER_INBOUND_INBOX_PAGE_V1 = 8,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_OPERATION_PREPARE_EXACT_NEXT_TRANSITION_V1 = 9,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_OPERATION_RECOVER_PREPARED_TRANSITION_V1 = 10,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_OPERATION_ABANDON_UNCOMMITTED_PREPARED_TRANSITION_V1 = 11,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_OPERATION_COMMIT_VERIFIED_CANDIDATE_V1 = 12,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_OPERATION_RECOVER_TERMINAL_COMMIT_CERTIFICATE_V1 = 13,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_OPERATION_INSTALL_FINAL_COMMIT_WRAPPER_V1 = 14,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_OPERATION_RECOVER_INSTALLED_ENVELOPE_OR_STATE_PROOF_V1 = 15,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_OPERATION_SIGN_RECEIVE_ACKNOWLEDGEMENT_V1 = 16,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_OPERATION_RELEASE_OUTBOX_ENTRY_V1 = 17,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_OPERATION_READ_TRUSTED_TIME_OR_LEASE_V1 = 18,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_OPERATION_PREPARE_MINT_AUTHORIZATION_V1 = 19,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_OPERATION_RECOVER_MINT_AUTHORIZATION_V1 = 20,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_OPERATION_VERIFY_AUTHORIZATION_AND_STAGE_MINT_CREDIT_V1 = 21,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_OPERATION_FOLD_RECEIVE_V1 = 22,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_OPERATION_READ_PENDING_CREDIT_WATERMARK_V1 = 23,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_OPERATION_ROTATE_HARDWARE_EPOCH_V1 = 24
} ConnectNoritoOfflineCashDeviceOperationV1;

// Values are encoded in the response frame's one-byte status field.
typedef enum ConnectNoritoOfflineCashDeviceStatusV1 {
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_STATUS_SUCCESS_V1 = 0,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_STATUS_UNAVAILABLE_V1 = 1,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_STATUS_STALE_OR_CONCURRENT_V1 = 2,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_STATUS_BINDING_MISMATCH_V1 = 3,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_STATUS_TRUSTED_TIME_REJECTED_V1 = 4,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_STATUS_REJECTED_V1 = 5,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_STATUS_MISSING_V1 = 6,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_STATUS_CONFLICT_V1 = 7,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_STATUS_CORRUPT_V1 = 8,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_STATUS_MALFORMED_REQUEST_V1 = 9,
  CONNECT_NORITO_OFFLINE_CASH_DEVICE_STATUS_RECOVERY_REQUIRED_V1 = 10
} ConnectNoritoOfflineCashDeviceStatusV1;

// Generic builds deliberately expose no monetary software fallback. These functions return
// CONNECT_NORITO_ERR_OFFLINE_CASH_DEVICE_UNAVAILABLE_V1 until replaced by a qualified,
// attested non-forking platform provider.
int32_t connect_norito_offline_cash_device_capabilities_v1(
    uint8_t* output, size_t output_capacity);
int32_t connect_norito_offline_cash_device_execute_v1(
    const uint8_t* command, size_t command_len,
    uint8_t* output, size_t output_capacity, size_t* output_len);

// ---------------- Privacy compiled-profile native FFI ----------------
// Output buffers are Norito V1 archives allocated by the bridge and must be
// released with `iroha_privacy_free_buffer`, which zeroizes privacy output
// memory before release.
// The catalog describes only profiles compiled into this binary. It contains
// no committed height, consensus policy, activation, lifecycle, or readiness
// state. Fetch a fresh PrivacyCapabilitySnapshotV1 from live Torii before
// treating a protocol as ready for proof submission.
typedef enum iroha_privacy_compiled_profile_catalog_validation_status_v1 {
    IROHA_PRIVACY_COMPILED_PROFILE_CATALOG_VALID_V1 = 0,
    IROHA_PRIVACY_COMPILED_PROFILE_CATALOG_NULL_POINTER_V1 = 1,
    IROHA_PRIVACY_COMPILED_PROFILE_CATALOG_EMPTY_V1 = 2,
    IROHA_PRIVACY_COMPILED_PROFILE_CATALOG_ARCHIVE_TOO_LARGE_V1 = 3,
    IROHA_PRIVACY_COMPILED_PROFILE_CATALOG_DECODE_RESOURCE_LIMIT_V1 = 4,
    IROHA_PRIVACY_COMPILED_PROFILE_CATALOG_SCHEMA_MISMATCH_V1 = 5,
    IROHA_PRIVACY_COMPILED_PROFILE_CATALOG_NON_CANONICAL_V1 = 6,
    IROHA_PRIVACY_COMPILED_PROFILE_CATALOG_MALFORMED_ARCHIVE_V1 = 7,
    IROHA_PRIVACY_COMPILED_PROFILE_CATALOG_INVALID_CATALOG_V1 = 8
} iroha_privacy_compiled_profile_catalog_validation_status_v1;

int32_t iroha_privacy_compiled_profile_catalog_v1(
    uint8_t** out_ptr,
    unsigned long* out_len);

int32_t iroha_privacy_validate_compiled_profile_catalog_v1(
    const uint8_t* archive_ptr,
    unsigned long archive_len);

// Complete Rust-derived canonical bytes through signed-transaction and hash
// layers for all twelve first-release rows. The archive is accepted only when
// it is byte-identical to the bundle compiled from the typed Rust fixtures.
typedef enum iroha_privacy_exact12_fixture_validation_status_v1 {
    IROHA_PRIVACY_EXACT12_FIXTURE_VALID_V1 = 0,
    IROHA_PRIVACY_EXACT12_FIXTURE_NULL_POINTER_V1 = 1,
    IROHA_PRIVACY_EXACT12_FIXTURE_EMPTY_V1 = 2,
    IROHA_PRIVACY_EXACT12_FIXTURE_ARCHIVE_TOO_LARGE_V1 = 3,
    IROHA_PRIVACY_EXACT12_FIXTURE_DECODE_RESOURCE_LIMIT_V1 = 4,
    IROHA_PRIVACY_EXACT12_FIXTURE_SCHEMA_MISMATCH_V1 = 5,
    IROHA_PRIVACY_EXACT12_FIXTURE_NON_CANONICAL_V1 = 6,
    IROHA_PRIVACY_EXACT12_FIXTURE_MALFORMED_ARCHIVE_V1 = 7,
    IROHA_PRIVACY_EXACT12_FIXTURE_INVALID_BUNDLE_V1 = 8
} iroha_privacy_exact12_fixture_validation_status_v1;

int32_t iroha_privacy_exact12_fixture_bundle_v1(
    uint8_t** out_ptr,
    unsigned long* out_len);

int32_t iroha_privacy_validate_exact12_fixture_bundle_v1(
    const uint8_t* archive_ptr,
    unsigned long archive_len);

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

// Validates one canonical appeal-finance CancelAssetLock V1 payload and
// returns ValidationOutcomeV1 JSON. The output must be released with
// connect_norito_free.
int32_t connect_norito_sorafs_reference_validate_appeal_finance_cancel_asset_lock_json(
    const uint8_t* bytes_ptr,
    unsigned long bytes_len,
    const uint8_t* label_ptr,
    unsigned long label_len,
    uint64_t generated_at,
    uint8_t** out_json_ptr,
    unsigned long* out_json_len);

// Validates a bounded heterogeneous fixture bundle and all supported
// manifest/provider/proof/orderbook cross-links. The output must be released
// with connect_norito_free.
int32_t connect_norito_sorafs_reference_validate_bundle_json(
    const ConnectNoritoSorafsReferenceBundlePayload* payloads_ptr,
    size_t payloads_len,
    uint64_t now,
    uint64_t generated_at,
    uint8_t** out_json_ptr,
    size_t* out_json_len);

// Validates one GovernanceLogNodeV1 against its required exact 32-byte CID and
// returns ValidationOutcomeV1 JSON. The output must be released with
// connect_norito_free.
int32_t connect_norito_sorafs_reference_validate_governance_json(
    const uint8_t* bytes_ptr,
    size_t bytes_len,
    const uint8_t* label_ptr,
    size_t label_len,
    const uint8_t* expected_node_cid_ptr,
    size_t expected_node_cid_len,
    uint64_t generated_at,
    uint8_t** out_json_ptr,
    size_t* out_json_len);

// Validates one GovernanceDagBlockV1 and returns ValidationOutcomeV1 JSON.
// expected_block_cid must be empty or exactly 32 bytes.
// The output must be released with connect_norito_free.
int32_t connect_norito_sorafs_reference_validate_governance_dag_block_json(
    const uint8_t* bytes_ptr,
    size_t bytes_len,
    const uint8_t* label_ptr,
    size_t label_len,
    const uint8_t* expected_block_cid_ptr,
    size_t expected_block_cid_len,
    uint64_t generated_at,
    uint8_t** out_json_ptr,
    size_t* out_json_len);

// Validates a signed GovernanceDagHeadV1 against an ordered root history or
// exact checkpoint-anchored tail (at most 64 supplied blocks).
// The output must be released with connect_norito_free.
int32_t connect_norito_sorafs_reference_validate_governance_dag_head_chain_json(
    const uint8_t* head_ptr,
    size_t head_len,
    const uint8_t* head_label_ptr,
    size_t head_label_len,
    const ConnectNoritoSorafsReferenceInput* blocks_ptr,
    size_t blocks_len,
    uint64_t generated_at,
    uint8_t** out_json_ptr,
    size_t* out_json_len);

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
    const uint8_t* price_per_gib_ptr,
    unsigned long price_per_gib_len,
    uint64_t quantity_gib,
    uint64_t remaining_gib,
    const uint8_t* owner_account_ptr,
    unsigned long owner_account_len,
    const uint8_t* provider_id_ptr,
    unsigned long provider_id_len,
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
    const uint8_t* xor_debited_ptr,
    unsigned long xor_debited_len,
    const uint8_t* provider_credit_ptr,
    unsigned long provider_credit_len,
    const uint8_t* fee_amount_ptr,
    unsigned long fee_amount_len,
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

// ---------------- Confidential-note derivation ----------------
// All digests are canonical 32-byte Pasta scalar encodings. Every derivation
// is owned by iroha_core's complete V3 Poseidon permutation; SDK-local
// substitutes are not part of the first-release contract. Caller-owned output
// buffers must be exactly 32 bytes. Zero is success; failures use the common
// bridge codes (-1 null pointer, -2 UTF-8, -11 output length, -15 invalid
// confidential derivation).
uint32_t connect_norito_confidential_note_derivation_revision_v3(void);
int32_t connect_norito_confidential_default_diversifier_v3(
    uint8_t* out_digest_ptr, unsigned long out_digest_len);
int32_t connect_norito_confidential_diversifier_derive_v3(
    const uint8_t* seed_ptr, unsigned long seed_len,
    uint8_t* out_digest_ptr, unsigned long out_digest_len);
int32_t connect_norito_confidential_owner_tag_derive_v3(
    const uint8_t* spend_key_ptr, unsigned long spend_key_len,
    const uint8_t* diversifier_ptr, unsigned long diversifier_len,
    uint8_t* out_digest_ptr, unsigned long out_digest_len);
int32_t connect_norito_confidential_asset_tag_derive_v3(
    const uint8_t* asset_ptr, unsigned long asset_len,
    uint8_t* out_digest_ptr, unsigned long out_digest_len);
int32_t connect_norito_confidential_network_tag_derive_v3(
    const uint8_t* network_id_ptr, unsigned long network_id_len,
    uint8_t* out_digest_ptr, unsigned long out_digest_len);
int32_t connect_norito_confidential_note_commitment_derive_v3(
    const uint8_t* asset_ptr, unsigned long asset_len,
    const uint8_t* amount_ptr, unsigned long amount_len,
    const uint8_t* rho_ptr, unsigned long rho_len,
    const uint8_t* owner_tag_ptr, unsigned long owner_tag_len,
    uint8_t* out_digest_ptr, unsigned long out_digest_len);
int32_t connect_norito_confidential_nullifier_derive_v3(
    const uint8_t* network_id_ptr, unsigned long network_id_len,
    const uint8_t* asset_ptr, unsigned long asset_len,
    const uint8_t* spend_key_ptr, unsigned long spend_key_len,
    const uint8_t* rho_ptr, unsigned long rho_len,
    uint8_t* out_digest_ptr, unsigned long out_digest_len);
// Merkle-path output is root[32] || siblings[16][32] || directions[16].
int32_t connect_norito_confidential_merkle_path_derive_v3(
    const uint8_t* commitments_ptr, unsigned long commitments_len,
    uint64_t leaf_index,
    uint8_t* out_path_ptr, unsigned long out_path_len);
int32_t connect_norito_confidential_merkle_path_verify_v3(
    const uint8_t* commitment_ptr, unsigned long commitment_len,
    uint64_t leaf_index,
    const uint8_t* siblings_ptr, unsigned long siblings_len,
    const uint8_t* directions_ptr, unsigned long directions_len,
    const uint8_t* root_ptr, unsigned long root_len);
// Advance output is final_root[32] || next_zero_root[32] ||
// next_zero_siblings[16][32] || next_zero_directions[16].
int32_t connect_norito_confidential_merkle_path_advance_v3(
    uint64_t leaf_index,
    const uint8_t* siblings_ptr, unsigned long siblings_len,
    const uint8_t* directions_ptr, unsigned long directions_len,
    const uint8_t* root_ptr, unsigned long root_len,
    const uint8_t* commitment_ptr, unsigned long commitment_len,
    uint8_t* out_ptr, unsigned long out_len);

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

int32_t connect_norito_decode_control_open_network_id(
    const uint8_t* inp, unsigned long inp_len,
    uint8_t** out_ptr, unsigned long* out_len);

int32_t connect_norito_decode_control_approve_permissions_json(
    const uint8_t* inp, unsigned long inp_len,
    uint8_t** out_ptr, unsigned long* out_len);

int32_t connect_norito_decode_control_approve_proof_json(
    const uint8_t* inp, unsigned long inp_len,
    uint8_t** out_ptr, unsigned long* out_len);

// ---------------- Exact Connect identity and approval crypto ----------------
int32_t connect_norito_connect_derive_session_id(
    const uint8_t* network_id, unsigned long network_id_len,
    const uint8_t* app_pk, unsigned long app_pk_len,
    const uint8_t* nonce, unsigned long nonce_len,
    uint8_t* out_sid, unsigned long out_sid_len);

int32_t connect_norito_connect_relay_auth_hash(
    const uint8_t* sid, unsigned long sid_len,
    const char* relay_token, unsigned long relay_token_len,
    uint8_t* out_hash, unsigned long out_hash_len);

int32_t connect_norito_connect_approval_preimage(
    const uint8_t* network_id, unsigned long network_id_len,
    const uint8_t* sid, unsigned long sid_len,
    const uint8_t* app_pk, unsigned long app_pk_len,
    const uint8_t* nonce, unsigned long nonce_len,
    const uint8_t* wallet_pk, unsigned long wallet_pk_len,
    const char* account_id, unsigned long account_id_len,
    const uint8_t* permissions_json, unsigned long permissions_len,
    const uint8_t* proof_json, unsigned long proof_len,
    const char* relay_token, unsigned long relay_token_len,
    uint8_t** out_ptr, unsigned long* out_len);

int32_t connect_norito_connect_verify_approval(
    const uint8_t* network_id, unsigned long network_id_len,
    const uint8_t* sid, unsigned long sid_len,
    const uint8_t* app_pk, unsigned long app_pk_len,
    const uint8_t* nonce, unsigned long nonce_len,
    const uint8_t* wallet_pk, unsigned long wallet_pk_len,
    const char* account_id, unsigned long account_id_len,
    const uint8_t* permissions_json, unsigned long permissions_len,
    const uint8_t* proof_json, unsigned long proof_len,
    const char* relay_token, unsigned long relay_token_len,
    const char* algorithm, unsigned long algorithm_len,
    const uint8_t* signature, unsigned long signature_len);

int32_t connect_norito_connect_generate_keypair(uint8_t* out_pk, uint8_t* out_sk);
int32_t connect_norito_connect_public_from_private(
    const uint8_t* private_key, uint8_t* out_pk);
int32_t connect_norito_connect_derive_keys(
    const uint8_t* private_key,
    const uint8_t* peer_public_key,
    const uint8_t* sid,
    uint8_t* out_app_key,
    uint8_t* out_wallet_key);
int32_t connect_norito_connect_encrypt_envelope(
    const uint8_t* key,
    const uint8_t* sid,
    uint8_t dir,
    const uint8_t* envelope, unsigned long envelope_len,
    uint8_t** out_ptr, unsigned long* out_len);
int32_t connect_norito_connect_decrypt_ciphertext(
    const uint8_t* key,
    const uint8_t* frame, unsigned long frame_len,
    uint8_t** out_ptr, unsigned long* out_len);

// ---------------- Extended control encoders ----------------
int32_t connect_norito_encode_control_open_ext(
    const uint8_t* sid,
    uint8_t dir,
    uint64_t seq,
    const uint8_t* app_pk, unsigned long app_pk_len,
    const uint8_t* nonce, unsigned long nonce_len,
    const uint8_t* app_meta_json, unsigned long app_meta_len,
    const uint8_t* network_id, unsigned long network_id_len,
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

// Validate and canonicalize one bare ConfidentialMemoEnvelopeV1 wire.
// Returns -1 for null pointers, -2 when the capped wire is too large, and -3
// for any malformed, non-canonical, legacy, truncated, or trailing bytes.
int32_t connect_norito_validate_confidential_memo_envelope_v1(
    const uint8_t* envelope,
    unsigned long envelope_len,
    uint8_t** out_ptr, unsigned long* out_len);

// Generate one ML-KEM-768 (suite 0) or ML-KEM-1024 (suite 1) memo keypair.
int32_t connect_norito_generate_confidential_memo_keypair_v1(
    uint8_t suite_tag,
    uint8_t** public_key_out, unsigned long* public_key_len_out,
    uint8_t** secret_key_out, unsigned long* secret_key_len_out);

// Zeroizes and releases a secret-key or opened-plaintext output from the memo
// functions. The original returned length is mandatory.
void connect_norito_confidential_memo_secret_free_v1(
    uint8_t* secret_key, unsigned long secret_key_len);

// Seal plaintext for 1..8 same-suite recipient public keys. The packed key
// length must equal recipient_count times the suite's exact public-key length.
int32_t connect_norito_seal_confidential_memo_v1(
    uint8_t suite_tag,
    const uint8_t* recipient_public_keys,
    unsigned long recipient_public_keys_len,
    uint8_t recipient_count,
    const uint8_t* plaintext,
    unsigned long plaintext_len,
    uint8_t** out_ptr, unsigned long* out_len);

// Open one canonical bare memo wire for an exact-suite recipient secret key.
int32_t connect_norito_open_confidential_memo_v1(
    uint8_t suite_tag,
    const uint8_t* recipient_secret_key,
    unsigned long recipient_secret_key_len,
    const uint8_t* envelope,
    unsigned long envelope_len,
    uint8_t** out_ptr, unsigned long* out_len);

// Transaction encoder error codes:
//   0  success
//  -1  null pointer provided for input/output
//  -2  invalid UTF-8 in input strings
//  -3  network_id parse failure (requires one exact canonical checksummed
//      `hash:<64 uppercase hex>#<CRC16>` genesis-hash literal)
//  -4  authority account id parse failure
//  -5  asset definition id parse failure
//  -6  destination account id parse failure
//  -7  quantity parse failure
//  -8  invalid TTL (zero when present)
//  -9  private key parse failure
// -10  allocation failure while writing output
// -11  provided hash buffer shorter than 32 bytes
// -31  invalid nonce (zero when present)
// -34  missing or invalid typed fee-payment JSON
int32_t connect_norito_encode_transfer_signed_transaction(
    const char* network_id, unsigned long network_id_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    uint32_t nonce,
    uint8_t nonce_present,
    const char* asset_definition, unsigned long asset_definition_len,
    const char* quantity, unsigned long quantity_len,
    const char* destination, unsigned long destination_len,
    const uint8_t* fee_payment_json, unsigned long fee_payment_json_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_transfer_signed_transaction_alg(
    const char* network_id, unsigned long network_id_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    uint32_t nonce,
    uint8_t nonce_present,
    const char* asset_definition, unsigned long asset_definition_len,
    const char* quantity, unsigned long quantity_len,
    const char* destination, unsigned long destination_len,
    const uint8_t* fee_payment_json, unsigned long fee_payment_json_len,
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

int32_t connect_norito_encode_register_zk_asset_signed_transaction(
    const char* network_id, unsigned long network_id_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* asset_definition, unsigned long asset_definition_len,
    const char* vk_unshield, unsigned long vk_unshield_len, uint8_t vk_unshield_present,
    const char* vk_shield, unsigned long vk_shield_len, uint8_t vk_shield_present,
    const uint8_t* fee_payment_json, unsigned long fee_payment_json_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_register_zk_asset_signed_transaction_alg(
    const char* network_id, unsigned long network_id_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* asset_definition, unsigned long asset_definition_len,
    const char* vk_unshield, unsigned long vk_unshield_len, uint8_t vk_unshield_present,
    const char* vk_shield, unsigned long vk_shield_len, uint8_t vk_shield_present,
    const uint8_t* fee_payment_json, unsigned long fee_payment_json_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t algorithm,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_multisig_register_signed_transaction(
    const char* network_id, unsigned long network_id_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* spec_json, unsigned long spec_json_len,
    const char* account_id, unsigned long account_id_len,
    const uint8_t* fee_payment_json, unsigned long fee_payment_json_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_multisig_register_signed_transaction_alg(
    const char* network_id, unsigned long network_id_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* spec_json, unsigned long spec_json_len,
    const char* account_id, unsigned long account_id_len,
    const uint8_t* fee_payment_json, unsigned long fee_payment_json_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t algorithm,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_claim_identifier_signed_transaction(
    const char* network_id, unsigned long network_id_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* account_id, unsigned long account_id_len,
    const char* receipt_json, unsigned long receipt_json_len,
    const uint8_t* fee_payment_json, unsigned long fee_payment_json_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_claim_identifier_signed_transaction_alg(
    const char* network_id, unsigned long network_id_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* account_id, unsigned long account_id_len,
    const char* receipt_json, unsigned long receipt_json_len,
    const uint8_t* fee_payment_json, unsigned long fee_payment_json_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t algorithm,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_set_key_value_signed_transaction(
    const char* network_id, unsigned long network_id_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    uint8_t target_kind,
    const char* object_id, unsigned long object_len,
    const char* key, unsigned long key_len,
    const uint8_t* value_json, unsigned long value_json_len,
    const uint8_t* fee_payment_json, unsigned long fee_payment_json_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_set_key_value_signed_transaction_alg(
    const char* network_id, unsigned long network_id_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    uint8_t target_kind,
    const char* object_id, unsigned long object_len,
    const char* key, unsigned long key_len,
    const uint8_t* value_json, unsigned long value_json_len,
    const uint8_t* fee_payment_json, unsigned long fee_payment_json_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t algorithm,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_remove_key_value_signed_transaction(
    const char* network_id, unsigned long network_id_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    uint8_t target_kind,
    const char* object_id, unsigned long object_len,
    const char* key, unsigned long key_len,
    const uint8_t* fee_payment_json, unsigned long fee_payment_json_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_remove_key_value_signed_transaction_alg(
    const char* network_id, unsigned long network_id_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    uint8_t target_kind,
    const char* object_id, unsigned long object_len,
    const char* key, unsigned long key_len,
    const uint8_t* fee_payment_json, unsigned long fee_payment_json_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t algorithm,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

/* Canonical first-release deploy proposal signer. Both hash lengths must be 32,
 * abi_version must be 1, and provenance_present selects exactly either both
 * provenance strings or neither. Provenance is never synthesized from the
 * transaction signing key. */
int32_t connect_norito_encode_governance_propose_deploy_v1_signed_transaction(
    const char* network_id, unsigned long network_id_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* contract_address, unsigned long contract_address_len,
    const uint8_t* code_hash, unsigned long code_hash_len,
    const uint8_t* abi_hash, unsigned long abi_hash_len,
    uint16_t abi_version,
    const char* provenance_signer, unsigned long provenance_signer_len,
    const char* provenance_signature, unsigned long provenance_signature_len,
    uint8_t provenance_present,
    const uint8_t* fee_payment_json, unsigned long fee_payment_json_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_governance_propose_deploy_v1_signed_transaction_alg(
    const char* network_id, unsigned long network_id_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* contract_address, unsigned long contract_address_len,
    const uint8_t* code_hash, unsigned long code_hash_len,
    const uint8_t* abi_hash, unsigned long abi_hash_len,
    uint16_t abi_version,
    const char* provenance_signer, unsigned long provenance_signer_len,
    const char* provenance_signature, unsigned long provenance_signature_len,
    uint8_t provenance_present,
    const uint8_t* fee_payment_json, unsigned long fee_payment_json_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t algorithm,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_governance_cast_plain_ballot_signed_transaction(
    const char* network_id, unsigned long network_id_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* referendum_id, unsigned long referendum_id_len,
    const char* owner, unsigned long owner_len,
    const char* amount, unsigned long amount_len,
    uint64_t duration_blocks,
    uint8_t direction,
    const uint8_t* fee_payment_json, unsigned long fee_payment_json_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_governance_cast_plain_ballot_signed_transaction_alg(
    const char* network_id, unsigned long network_id_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* referendum_id, unsigned long referendum_id_len,
    const char* owner, unsigned long owner_len,
    const char* amount, unsigned long amount_len,
    uint64_t duration_blocks,
    uint8_t direction,
    const uint8_t* fee_payment_json, unsigned long fee_payment_json_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t algorithm,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_governance_cast_zk_ballot_signed_transaction(
    const char* network_id, unsigned long network_id_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* election_id, unsigned long election_id_len,
    const char* proof_b64, unsigned long proof_b64_len,
    const uint8_t* public_inputs_json, unsigned long public_inputs_len,
    const uint8_t* fee_payment_json, unsigned long fee_payment_json_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_governance_cast_zk_ballot_signed_transaction_alg(
    const char* network_id, unsigned long network_id_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* election_id, unsigned long election_id_len,
    const char* proof_b64, unsigned long proof_b64_len,
    const uint8_t* public_inputs_json, unsigned long public_inputs_len,
    const uint8_t* fee_payment_json, unsigned long fee_payment_json_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t algorithm,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_mint_signed_transaction(
    const char* network_id, unsigned long network_id_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    uint32_t nonce,
    uint8_t nonce_present,
    const char* asset_definition, unsigned long asset_definition_len,
    const char* quantity, unsigned long quantity_len,
    const char* destination, unsigned long destination_len,
    const uint8_t* fee_payment_json, unsigned long fee_payment_json_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_mint_signed_transaction_alg(
    const char* network_id, unsigned long network_id_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    uint32_t nonce,
    uint8_t nonce_present,
    const char* asset_definition, unsigned long asset_definition_len,
    const char* quantity, unsigned long quantity_len,
    const char* destination, unsigned long destination_len,
    const uint8_t* fee_payment_json, unsigned long fee_payment_json_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t algorithm,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_burn_signed_transaction(
    const char* network_id, unsigned long network_id_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    uint32_t nonce,
    uint8_t nonce_present,
    const char* asset_definition, unsigned long asset_definition_len,
    const char* quantity, unsigned long quantity_len,
    const char* destination, unsigned long destination_len,
    const uint8_t* fee_payment_json, unsigned long fee_payment_json_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_burn_signed_transaction_alg(
    const char* network_id, unsigned long network_id_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    uint32_t nonce,
    uint8_t nonce_present,
    const char* asset_definition, unsigned long asset_definition_len,
    const char* quantity, unsigned long quantity_len,
    const char* destination, unsigned long destination_len,
    const uint8_t* fee_payment_json, unsigned long fee_payment_json_len,
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
