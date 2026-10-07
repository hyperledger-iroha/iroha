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

#define CONNECT_NORITO_BRIDGE_ABI_VERSION 26

#define CONNECT_NORITO_ERR_ACCOUNT_ADDRESS -200
#define CONNECT_NORITO_ERR_UNSUPPORTED_ALGORITHM -21
#define CONNECT_NORITO_ERR_SORAFS_REFERENCE -114
#define CONNECT_NORITO_ERR_DETACHED_TRANSACTION_SCAFFOLD -501
#define CONNECT_NORITO_ERR_DETACHED_TRANSACTION_SIGNATURE -502
#define CONNECT_NORITO_ERR_CANONICAL_JSON -503
#define CONNECT_NORITO_ERR_VALIDATION_FEE_POLICY_PROOF -504
#define CONNECT_NORITO_ERR_PARLIAMENT_TIMED_OVN -505
#define CONNECT_NORITO_ERR_RETAIL_FEE_ASSESSMENT -506
#define CONNECT_NORITO_ERR_PRIVATE_SETTLEMENT_RESPONSE -507
#define CONNECT_NORITO_ERR_COMMITTED_TRANSACTION_INCLUSION -508
#define CONNECT_NORITO_ERR_COMMITTED_TRANSACTION_QUERY -509
#define CONNECT_NORITO_ERR_CONNECT_IDENTITY -410
#define CONNECT_NORITO_ERR_CONNECT_APPROVAL -411

#define CONNECT_NORITO_PRIVATE_SETTLEMENT_REQUEST_MAX_BYTES_V1 1048576
#define CONNECT_NORITO_PRIVATE_SETTLEMENT_RESPONSE_MAX_BYTES_V1 33554432

#define CONNECT_NORITO_PARLIAMENT_TIMED_OVN_SEED_BYTES_V1 32
#define CONNECT_NORITO_PARLIAMENT_TIMED_OVN_TRUST_ANCHOR_BYTES_V1 32
#define CONNECT_NORITO_PARLIAMENT_TIMED_OVN_CASTING_PROOF_MAX_BYTES_V1 8388608
#define CONNECT_NORITO_PARLIAMENT_TIMED_OVN_CASTING_PROOF_PAGE_SUMMARY_BYTES_V1 41

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

// Authenticate one selective current CommittedTransaction QueryResponse against
// an independently selected NetworkId, chain UTF-8 label (1..1024 bytes), canonical
// native checkpoint (at most 68 MiB), and exact requested transaction hash.
// The bounded JSON array contains 1..4096 consecutive SumeragiFinalityProof values,
// beginning at that checkpoint, at most 16 MiB. No scalar trust anchor is accepted.
// The verified native capability binds the original full wire and result before
// row inclusion. A genuine rejected execution returns success with result_ok=0.
// On success, free both the canonical row (at most 4 MiB) and promoted canonical
// checkpoint with connect_norito_free. All outputs are cleared on failure;
// output slots must be distinct and must not overlap input storage.
int32_t connect_norito_verify_committed_transaction_inclusion_v1(
    const uint8_t* response,
    unsigned long response_len,
    const uint8_t* native_finality_proof_chain_json,
    unsigned long native_finality_proof_chain_json_len,
    const uint8_t* expected_network_id,
    unsigned long expected_network_id_len,
    const uint8_t* expected_chain_utf8,
    unsigned long expected_chain_utf8_len,
    const uint8_t* trusted_checkpoint,
    unsigned long trusted_checkpoint_len,
    const uint8_t* expected_transaction_hash,
    unsigned long expected_transaction_hash_len,
    uint8_t** out_row,
    unsigned long* out_row_len,
    uint8_t* out_output_hash_32,
    uint8_t* out_block_hash_32,
    uint64_t* out_block_height,
    uint8_t* out_result_ok,
    uint8_t** out_checkpoint,
    unsigned long* out_checkpoint_len);

// Exact committed-row decoder returning an *untrusted routing hint* for locating
// the carrier among consecutive native finality proofs. Status 0 means one matching
// row and writes its block hash; status 1 means a canonical empty committed-row
// page and leaves output zeroed; -508 rejects malformed, foreign, multirow or
// wrong-transaction evidence. A status-0 hint never authenticates the block or
// output: verify it with connect_norito_verify_committed_transaction_inclusion_v1.
int32_t connect_norito_committed_transaction_candidate_block_hash_v1(
    const uint8_t* response,
    unsigned long response_len,
    const uint8_t* expected_transaction_hash,
    unsigned long expected_transaction_hash_len,
    uint8_t* out_block_hash_32);

// Build the sole current wallet-self committed-transaction read: signed
// FindTransactions with canonical AND(authority_eq, entry_eq), default selector
// and params, a caller-supplied unique 32-byte nonce, and fixed 100-second TTL.
// The external Ed25519 signer signs the returned exact 32-byte payload hash.
// Both calls must receive identical arguments; finalization verifies the
// signature against the authority's single controller. The signed query is
// versioned Norito for one POST /v1/query; Torii nonce replay rules prohibit retry.
int32_t connect_norito_committed_transaction_query_payload_hash_v1(
    const uint8_t* network_id,
    unsigned long network_id_len,
    const uint8_t* authority_utf8,
    unsigned long authority_utf8_len,
    const uint8_t* transaction_hash,
    unsigned long transaction_hash_len,
    uint64_t creation_time_ms,
    const uint8_t* nonce_32,
    unsigned long nonce_len,
    uint8_t* out_payload_hash_32);

int32_t connect_norito_committed_transaction_query_finalize_v1(
    const uint8_t* network_id,
    unsigned long network_id_len,
    const uint8_t* authority_utf8,
    unsigned long authority_utf8_len,
    const uint8_t* transaction_hash,
    unsigned long transaction_hash_len,
    uint64_t creation_time_ms,
    const uint8_t* nonce_32,
    unsigned long nonce_len,
    const uint8_t* signature_64,
    unsigned long signature_len,
    uint8_t** out_signed_query,
    unsigned long* out_signed_query_len);

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

// Encodes POST /v1/validation-fee/policy/current/proof from one independently
// selected complete canonical native checkpoint. Its height is derived locally.
int32_t connect_norito_validation_fee_current_policy_proof_request_v1(
    const uint8_t* trusted_checkpoint,
    unsigned long trusted_checkpoint_len,
    uint8_t** out_request,
    unsigned long* out_request_len);

// Verifies native finality, ordinary writes, registry and immutable network/policy
// bindings. Returns projection JSON and the promoted canonical native checkpoint.
// Persist the pair atomically before paging. Outputs must be distinct, are both
// cleared on failure, and each must be freed with connect_norito_free on success.
int32_t connect_norito_validation_fee_current_policy_proof_verify_v1(
    const uint8_t* proof_norito,
    unsigned long proof_norito_len,
    const uint8_t* network_id,
    unsigned long network_id_len,
    const uint8_t* policy_chain_genesis_hash,
    unsigned long policy_chain_genesis_hash_len,
    const uint8_t* trusted_checkpoint,
    unsigned long trusted_checkpoint_len,
    uint8_t** out_projection_json,
    unsigned long* out_projection_json_len,
    uint8_t** out_promoted_checkpoint,
    unsigned long* out_promoted_checkpoint_len);

// Native retail fee codec. Output is cleared on failure; release successful
// output with connect_norito_free. Invalid input returns -506. These codecs do
// not replace policy finality verification or ledger assessment admission.
// Typed request JSON -> 32 raw intent hash bytes (at most 262144 input bytes).
int32_t connect_norito_retail_fee_intent_hash_v1(
    const uint8_t* input, unsigned long input_len,
    uint8_t** output, unsigned long* output_len);

// Typed assessment JSON -> canonical TRACE marker UTF-8 (4096-byte input/output bounds).
int32_t connect_norito_retail_fee_assessment_marker_v1(
    const uint8_t* input, unsigned long input_len,
    uint8_t** output, unsigned long* output_len);

// Canonical marker UTF-8 -> typed assessment JSON (4096-byte input/output bounds).
int32_t connect_norito_retail_fee_assessment_decode_v1(
    const uint8_t* input, unsigned long input_len,
    uint8_t** output, unsigned long* output_len);

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
// exactly 41 diagnostic bytes: big-endian u64 evaluated height, 32-byte evaluated context
// id, and canonical 0/1 more-available. Intermediate pages promote only the
// checkpoint; terminal pages also replay and bind the complete Core archive.
// Every successful call returns the complete canonical promoted checkpoint in
// separate owned output storage. Free every owned output with connect_norito_free.
// A scalar summary is never an input authority. All output slots are distinct.
int32_t connect_norito_parliament_timed_ovn_verify_casting_proof_page_v1(
    const uint8_t* proof_response_norito,
    unsigned long proof_response_norito_len,
    const uint8_t* network_id,
    unsigned long network_id_len,
    const uint8_t* trusted_checkpoint_norito,
    unsigned long trusted_checkpoint_norito_len,
    const uint8_t* expected_ballot_attempt_id,
    unsigned long expected_ballot_attempt_id_len,
    uint8_t* out_page_result,
    unsigned long out_page_result_len,
    uint8_t** out_checkpoint_norito,
    unsigned long* out_checkpoint_norito_len);

// Authenticates a terminal proof response against independently configured
// network/checkpoint/ballot anchors, then canonical-decodes and replay-validates
// its Core archive and rederives the authenticated compact binding.
int32_t connect_norito_parliament_timed_ovn_verify_casting_proof_v1(
    const uint8_t* proof_response_norito,
    unsigned long proof_response_norito_len,
    const uint8_t* network_id,
    unsigned long network_id_len,
    const uint8_t* trusted_checkpoint_norito,
    unsigned long trusted_checkpoint_norito_len,
    const uint8_t* expected_ballot_attempt_id,
    unsigned long expected_ballot_attempt_id_len,
    uint8_t** out_checkpoint_norito,
    unsigned long* out_checkpoint_norito_len);

// Verifies the same proof and archive before reading the exact 32-byte
// caller-keystore seed. The public record and complete promoted checkpoint are returned.
// The two output slots must be distinct and must not overlap input storage.
int32_t connect_norito_parliament_timed_ovn_registration_from_proof_v1(
    const uint8_t* proof_response_norito,
    unsigned long proof_response_norito_len,
    const uint8_t* network_id,
    unsigned long network_id_len,
    const uint8_t* trusted_checkpoint_norito,
    unsigned long trusted_checkpoint_norito_len,
    const uint8_t* expected_ballot_attempt_id,
    unsigned long expected_ballot_attempt_id_len,
    const char* authority,
    unsigned long authority_len,
    const uint8_t* keystore_seed,
    unsigned long keystore_seed_len,
    uint8_t** out_registration,
    unsigned long* out_registration_len,
    uint8_t** out_checkpoint_norito,
    unsigned long* out_checkpoint_norito_len);

// Verifies the same proof and archive before reading the seed, reconstructs the
// exact committed registration, and returns a survivor- and release-bound
// masked ballot. choice is 0 (Aye), 1 (Nay), or 2 (Abstain).
// The two output slots must be distinct and must not overlap input storage.
int32_t connect_norito_parliament_timed_ovn_ballot_from_proof_v1(
    const uint8_t* proof_response_norito,
    unsigned long proof_response_norito_len,
    const uint8_t* network_id,
    unsigned long network_id_len,
    const uint8_t* trusted_checkpoint_norito,
    unsigned long trusted_checkpoint_norito_len,
    const uint8_t* expected_ballot_attempt_id,
    unsigned long expected_ballot_attempt_id_len,
    const char* authority,
    unsigned long authority_len,
    const uint8_t* keystore_seed,
    unsigned long keystore_seed_len,
    uint8_t choice,
    uint8_t** out_ballot,
    unsigned long* out_ballot_len,
    uint8_t** out_checkpoint_norito,
    unsigned long* out_checkpoint_norito_len);

// ---------------- Chain discriminant helpers ----------------
// Thread-scoped overrides must be exited on the same thread and in LIFO order.
// enter returns zero on failure; exit returns zero on success and -1 on misuse.
uint64_t connect_norito_chain_discriminant_scope_enter(uint16_t discriminant);
int32_t connect_norito_chain_discriminant_scope_exit(uint64_t token);

// ---------------- Canonical domain identity ----------------
// Require exact native-canonical ASCII domain.dataspace text (maximum 127 bytes).
// No normalization is performed. Returns 0 when valid, 1 when invalid, -1 for
// null input, -2 for invalid UTF-8, and -3 for a native panic. Lengths outside
// 1..127 are rejected before input is read. No output allocation is returned.
int32_t connect_norito_domain_id_validate_v1(const char* input, unsigned long input_len);

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

// Authoritative canonical/semantic evidence validation, including all release,
// audit and deployment signatures. Only zero accepts. The caller separately
// authenticates Torii and matches all committed tuples to its local catalog.
typedef enum iroha_privacy_exact12_capability_validation_status_v1 {
    IROHA_PRIVACY_EXACT12_CAPABILITY_VALID_V1 = 0,
    IROHA_PRIVACY_EXACT12_CAPABILITY_NULL_POINTER_V1 = 1,
    IROHA_PRIVACY_EXACT12_CAPABILITY_EMPTY_V1 = 2,
    IROHA_PRIVACY_EXACT12_CAPABILITY_ARCHIVE_TOO_LARGE_V1 = 3,
    IROHA_PRIVACY_EXACT12_CAPABILITY_DECODE_RESOURCE_LIMIT_V1 = 4,
    IROHA_PRIVACY_EXACT12_CAPABILITY_SCHEMA_MISMATCH_V1 = 5,
    IROHA_PRIVACY_EXACT12_CAPABILITY_NON_CANONICAL_V1 = 6,
    IROHA_PRIVACY_EXACT12_CAPABILITY_MALFORMED_ARCHIVE_V1 = 7,
    IROHA_PRIVACY_EXACT12_CAPABILITY_INVALID_MANIFEST_V1 = 8
} iroha_privacy_exact12_capability_validation_status_v1;

int32_t iroha_privacy_validate_exact12_capability_manifest_v1(
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

// Encode the current signature algorithm and its exact signature bytes.
int32_t connect_norito_encode_envelope_sign_result_ok_with_alg(
    uint64_t seq,
    const char* alg_ptr, unsigned long alg_len,
    const uint8_t* sig_ptr, unsigned long sig_len,
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
// Confidential wallet prover: local proof generation, never ledger authorization.
// Revision 1; handles never repeat. At most 64 open provers and 64 prepared jobs.
// Status: 0 success; -1 malformed ABI/operation/state; -2 closed/consumed handle;
// -3 resource limit; -10 invalid key; -11 input count; -12 tree capacity;
// -13 path count; -14 path shape; -15 input index; -16 path/index mismatch;
// -17 duplicate input; -18 output count; -19 transfer amounts; -20 input amounts;
// -21 public amount; -22 change; -23 key preparation; -24 proof; -100 internal.
// Create/job_create zero the output handle on failure. Set inputs and outputs
// before exactly one tree-evidence call. Only actual inputs need paths.
// Amounts are unsigned (high << 64) | low. close blocks future job creation;
// existing jobs retain the native key until consumed or job_close. job_prove
// consumes its job on success/error and returns public Norito JSON with keys:
// relation, backend, proof_hex, root_hex, nullifiers_hex, output_commitments_hex.
// Public JSON is capped at 16 MiB, checked before native hexadecimal allocation.
// Relation is confidential_transfer, confidential_full_unshield, or
// confidential_change_unshield. Free JSON output with connect_norito_free.
uint32_t connect_norito_confidential_prover_revision_v1(void);
int32_t connect_norito_confidential_prover_create_v1(const uint8_t* network, unsigned long network_len, const uint8_t* asset, unsigned long asset_len, const uint8_t* key, unsigned long key_len, uint64_t* out_handle);
int32_t connect_norito_confidential_prover_close_v1(uint64_t handle);
int32_t connect_norito_confidential_prover_job_create_v1(uint64_t handle, uint8_t operation, const uint8_t* root, unsigned long root_len, uint64_t amount_low, uint64_t amount_high, uint64_t* out_job);
int32_t connect_norito_confidential_prover_job_input_v1(uint64_t job, uint64_t amount_low, uint64_t amount_high, const uint8_t* rho, unsigned long rho_len, const uint8_t* diversifier, unsigned long diversifier_len, uint64_t leaf_index);
int32_t connect_norito_confidential_prover_job_output_v1(uint64_t job, uint64_t amount_low, uint64_t amount_high, const uint8_t* rho, unsigned long rho_len, const uint8_t* owner, unsigned long owner_len);
int32_t connect_norito_confidential_prover_job_commitments_v1(uint64_t job, const uint8_t* leaves, unsigned long leaves_len);
int32_t connect_norito_confidential_prover_job_paths_v1(uint64_t job, const uint8_t* siblings, unsigned long siblings_len, const uint8_t* directions, unsigned long directions_len);
int32_t connect_norito_confidential_prover_job_prove_v1(uint64_t job, uint8_t** out_json, unsigned long* out_len);
int32_t connect_norito_confidential_prover_job_close_v1(uint64_t job);


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

// Return the current wallet signature algorithm label from an Approve frame.
int32_t connect_norito_decode_control_approve_sig_alg(
    const uint8_t* inp_ptr, unsigned long inp_len,
    char** out_alg_ptr, unsigned long* out_alg_len);

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
/* One wallet-signed CanReadAccountData(authority) grant (change=1) or revoke
 * (change=2) to an exact reporting account. The authority must be a 1-of-1
 * multisig account controlled by the supplied Ed25519 key. The output is a
 * canonical versioned SignedTransaction and its 32-byte transaction hash.
 * fee_payment_json is required and must contain explicit signed charge limits;
 * the reviewed release policy selects the exact authority or sponsor payer. */
int32_t connect_norito_account_read_permission_multisig_payload_hash(
    const char* network_id, unsigned long network_id_len,
    const char* authority, unsigned long authority_len,
    const char* reporting_account, unsigned long reporting_account_len,
    uint8_t change, uint64_t creation_time_ms,
    const uint8_t* fee_payment_json, unsigned long fee_payment_json_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_account_read_permission_multisig_finalize(
    const char* network_id, unsigned long network_id_len,
    const char* authority, unsigned long authority_len,
    const char* reporting_account, unsigned long reporting_account_len,
    uint8_t change, uint64_t creation_time_ms,
    const uint8_t* fee_payment_json, unsigned long fee_payment_json_len,
    const uint8_t* signature, unsigned long signature_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_account_read_permission_multisig_signed_transaction(
    const char* network_id, unsigned long network_id_len,
    const char* authority, unsigned long authority_len,
    const char* reporting_account, unsigned long reporting_account_len,
    uint8_t change, uint64_t creation_time_ms,
    const uint8_t* fee_payment_json, unsigned long fee_payment_json_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

int32_t connect_norito_encode_account_read_permission_multisig_signed_transaction_alg(
    const char* network_id, unsigned long network_id_len,
    const char* authority, unsigned long authority_len,
    const char* reporting_account, unsigned long reporting_account_len,
    uint8_t change, uint64_t creation_time_ms,
    const uint8_t* fee_payment_json, unsigned long fee_payment_json_len,
    const uint8_t* private_key, unsigned long private_key_len,
    uint8_t algorithm,
    uint8_t** out_signed_ptr, unsigned long* out_signed_len,
    uint8_t* out_hash_ptr, unsigned long out_hash_len);

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

/* Current native registry frame for an asset transfer; output is bounded JSON. */
int32_t connect_norito_encode_transfer_instruction_frame_v1(
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

int32_t connect_norito_encode_governance_update_plain_conviction_signed_transaction_alg(
    const char* network_id, unsigned long network_id_len,
    const char* authority, unsigned long authority_len,
    uint64_t creation_time_ms,
    uint64_t ttl_ms,
    uint8_t ttl_present,
    const char* referendum_id, unsigned long referendum_id_len,
    const char* owner, unsigned long owner_len,
    const char* amount, unsigned long amount_len,
    uint64_t duration_blocks,
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
// Process attempt storage is separate from caller-owned State execution leases.
// Every field is a finite ceiling; zero denies admission of that resource.
typedef struct {
    uint64_t host_bytes;
    uint64_t pinned_bytes;
    uint64_t device_bytes;
    uint64_t in_flight;
    uint64_t metadata_bytes;
    uint64_t observed_devices;
    uint64_t discovery_ordinals;
    uint64_t modules;
    uint64_t streams;
    uint64_t artifact_bytes;
} connect_norito_acceleration_resource_limits;

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
    connect_norito_acceleration_resource_limits resource_limits;
} connect_norito_acceleration_config;

// Exact current record lengths only: -3 is returned before pointer access for any
// other size. Returns 0 on requested-policy application or -2 for malformed data.
// Null cfg with cfg_len == 0 restores ordinary enabled defaults and finite limits.
// These V1 symbols replace the retired size-less exports; no aliases are shipped.
int32_t connect_norito_acceleration_config_set_v1(const connect_norito_acceleration_config* cfg, size_t cfg_len);
int32_t connect_norito_acceleration_config_get_v1(connect_norito_acceleration_config* out_cfg, size_t out_len);

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

int32_t connect_norito_acceleration_state_get_v1(connect_norito_acceleration_state* out_state, size_t out_len);

/* KAGEMUSHA wallet V1: opaque exclusive owner. No arbitrary-sign entry point.
 * Open requires an authenticated native runtime; unprovisioned runtimes return -4 (ArtifactsUnavailable).
 * Callback contexts must be thread-safe and remain valid from retain through release.
 * Outputs own canonical Norito/exact retained bytes; free them with connect_norito_free.
 * Failure: -1 input, -2 closed, -3 capacity, -4 artifacts, -5 unavailable, -6 uncertain,
 * -7 custody lost, -8 fold required, -9 proof rejected, -10 cancelled, -11 conflict,
 * -12 no space, -13 terminal/key lost, -100 internal (outcome must be reconciled).
 */
typedef struct {
    uint32_t tag; /* 0 success, 1 absent, 2 unavailable/not performed, 3 exists,
                     4 uncertain, 5 destination absent; interpreted per operation. */
    uint32_t reason; /* 0 locked, 1 first unlock, 2 busy, 3 I/O, 4 platform,
                       5 unusable key, 6 permanently invalidated. */
    int32_t code;
    size_t length;
} connect_norito_kagemusha_platform_reply_v1;
typedef struct {
    uint32_t version; /* exactly 1 */
    uint32_t anchor_policy; /* exactly 1: Apple Keychain; Android uses JNI */
    void* context;
    void (*retain)(void* context);
    void (*release)(void* context);
    /* op: 0 key probe, 1 generate (input32/aux profile1/2), 2 sign (input32),
       3 key delete, 4 anchor read, 5 anchor create, 6 update, 7 storage state,
       8 boot UUID UTF8, 9 prepare non-backup custody root UTF8,
       10 complete ascending unique nonzero32 key slots (at most4096, no truncation).
       slot32 is null for operations7..10. Never retain or exceed borrowed buffers. */
    void (*invoke)(void* context, uint32_t operation, const uint8_t* slot32,
                   const uint8_t* input, size_t input_len, uint32_t auxiliary,
                   uint8_t* output, size_t output_capacity,
                   connect_norito_kagemusha_platform_reply_v1* reply);
} connect_norito_kagemusha_platform_v1;
typedef struct {
    int32_t status; /* 0 unknown,1 complete,2 pending,3 not performed,4 archived,
                      5 delivery loss,6 idle,7 caught up,8 checkpoint,9 folded,
                      10 CreditStatus,11 preparing; setup only:12 original,
                      13 owned time challenge,14 time exchange retained;
                      open only:15 account challenge,16 admitted handle;17 activation transport; negative failure. */
    int32_t reason; /* failure platform reason, -1 if inapplicable */
    int32_t platform_code;
    uint64_t sequence_low;
    uint64_t sequence_high;
    uint32_t detail; /* checkpoint ordinal or not-performed reason0/1/2 */
    uint8_t* bytes;
    size_t length;
} connect_norito_kagemusha_wallet_result_v1;
/* Source-selected snapshot. All amounts are Native projections, never operation permission.
   An unfolded head owns value subject to P4; only bit1 admits folded_balance. Failure clears
   all projection fields. Optional field storage is meaningful only under its explicit bit. */
typedef struct { uint64_t low; uint64_t high; } connect_norito_kagemusha_wallet_u128_v1;
typedef struct {
    int32_t status;
    int32_t reason;
    int32_t platform_code;
    uint32_t lifecycle; /* Active1, Retiring2; Retiring can Send/Unload remaining value */
    uint32_t flags; /* bit0 verified fold exists; bit1 exact current head folded */
    uint8_t scheme[32];
    uint8_t wallet[32];
    uint8_t head[32];
    uint8_t credential[32];
    uint8_t folded_head[32];
    uint8_t folded_credential[32];
    connect_norito_kagemusha_wallet_u128_v1 sequence;
    connect_norito_kagemusha_wallet_u128_v1 balance;
    connect_norito_kagemusha_wallet_u128_v1 core_burned_total;
    connect_norito_kagemusha_wallet_u128_v1 known_burned_total;
    connect_norito_kagemusha_wallet_u128_v1 owned_balance;
    connect_norito_kagemusha_wallet_u128_v1 folded_balance;
    connect_norito_kagemusha_wallet_u128_v1 fold_backlog;
    connect_norito_kagemusha_wallet_u128_v1 folded_sequence;
    connect_norito_kagemusha_wallet_u128_v1 folded_burned_total;
} connect_norito_kagemusha_wallet_snapshot_v1_t;
/* Fixed writable output, no allocated bytes to free. Call on a worker; it may verify Ω. */
int32_t connect_norito_kagemusha_wallet_snapshot_v1(uint64_t handle, connect_norito_kagemusha_wallet_snapshot_v1_t* out);
uint32_t connect_norito_kagemusha_wallet_revision_v1(void);

/* Canonical Load transport DATA only. Does not verify finality/proofs or change value.
 * Three selectors are exactly32 bytes; payer is strict canonical I105 UTF-8<=1024;
 * unsigned canonical receipt<=512, canonical LoadFinality original<=16384.
 * Zero means exact canonical data/selection/payer/receipt-digest binding only.
 * No pointer is retained. Native wallet Load independently authorizes all value. */
int32_t connect_norito_kagemusha_wallet_load_original_validate_v1(
    const uint8_t *scheme, const uint8_t *wallet, const uint8_t *request,
    const uint8_t *payer, size_t payer_length,
    const uint8_t *receipt, size_t receipt_length,
    const uint8_t *finality, size_t finality_length);

/* Native startup supplies the runtime handle. Foreign originals do not select trust pins. */
typedef struct {
    const uint8_t* credential;
    size_t credential_length;
    const uint8_t* certificates;
    size_t certificates_length;
    const uint8_t* account;
    size_t account_length;
    const uint8_t* asset;
    size_t asset_length;
} connect_norito_kagemusha_wallet_open_request_v1;
/* Kind15: fresh account challenge32; kind16: admitted wallet handle in sequence_low. */
int32_t connect_norito_kagemusha_wallet_open_begin_v1(uint64_t runtime, const connect_norito_kagemusha_wallet_open_request_v1* request, connect_norito_kagemusha_wallet_result_v1* out);
int32_t connect_norito_kagemusha_wallet_open_finish_v1(uint64_t runtime, const uint8_t* signature, size_t length, connect_norito_kagemusha_wallet_result_v1* out);
int32_t connect_norito_kagemusha_wallet_open_cancel_v1(uint64_t runtime);
int32_t connect_norito_kagemusha_wallet_close_v1(uint64_t handle);
int32_t connect_norito_kagemusha_wallet_activity_v1(uint64_t handle, uint8_t foreground, uint8_t charging);
// Native lifecycle intent; no caller state, roots, proof, time or nonce. Selector meanings:
// Load0(receipt,finality), Send1(Request), Receive2(Payment,credential,certificates),
// Credential3/SchemePolicy4/Blacklist5/TimeAnchor6/QuotaShare7(update,certificates),
// Unload8(optional quote+certificates, positive amount), Retire9(no originals).
// Unused slots/amount are zero. request_id is a nonzero local retry identity.
// Original bounds are selector-specific; Payment remains <=10,000 bytes.
typedef struct {
    const uint8_t* request_id;
    uint32_t selector;
    connect_norito_kagemusha_wallet_u128_v1 amount;
    const uint8_t* first;
    size_t first_length;
    const uint8_t* second;
    size_t second_length;
    const uint8_t* third;
    size_t third_length;
} connect_norito_kagemusha_wallet_operation_request_v1;
// Setup through the same exclusive Native owner. Selectors:
// Bootstrap0(no originals), Offer1(positive amount), Request2(Offer, optional fee+certificate),
// Credited3(Credited), BeginTime4(no originals), FinishTime5(TimeAnchor, certificate, owned token),
// CancelTime6(owned token); envelope wrap7..10 / unwrap11..14 in Offer,Request,Payment,Credited order.
// Activation15(no inputs) returns kind17, exact durable Activate frame <=16,384 bytes.
// setup_id is exactly32 bytes: nonzero only for Offer/Request and all zero otherwise.
// Unused originals/amount/token are empty/zero. Original bounds are selected by Native;
// signer certificate frames are <=512 bytes. No caller clock, nonce, proof or signing body.
// Transport uses first only and returns canonical bytes; it grants no monetary verdict.
typedef struct {
    const uint8_t* setup_id;
    uint32_t selector;
    connect_norito_kagemusha_wallet_u128_v1 amount;
    uint64_t token;
    const uint8_t* first;
    size_t first_length;
    const uint8_t* second;
    size_t second_length;
    const uint8_t* third;
    size_t third_length;
} connect_norito_kagemusha_wallet_setup_request_v1;
// Setup results: Bootstrap/Credited preserve completion statuses; CancelTime returns idle6.
// 12=exact original bytes;13=Native-owned one-use time challenge (token in sequence_low);
// 14=direct time exchange retained. Token is valid only for the same open Native owner.
// Neither 12,13,14 nor17 is monetary completion;17 is not ledger activation confirmation. Result bytes use connect_norito_free.
int32_t connect_norito_kagemusha_wallet_setup_v1(uint64_t handle, const connect_norito_kagemusha_wallet_setup_request_v1* request, connect_norito_kagemusha_wallet_result_v1* out);
int32_t connect_norito_kagemusha_wallet_execute_v1(uint64_t handle, const connect_norito_kagemusha_wallet_operation_request_v1* request, connect_norito_kagemusha_wallet_result_v1* out);
// Result11=preparing (no Advance selected); result2=pending (irreversible Advance selected).
int32_t connect_norito_kagemusha_wallet_request_status_v1(uint64_t handle, const uint8_t* request_id32, connect_norito_kagemusha_wallet_result_v1* out);
int32_t connect_norito_kagemusha_wallet_retry_v1(uint64_t handle, const uint8_t* operation32, connect_norito_kagemusha_wallet_result_v1* out);
int32_t connect_norito_kagemusha_wallet_resume_v1(uint64_t handle, connect_norito_kagemusha_wallet_result_v1* out);
int32_t connect_norito_kagemusha_wallet_fold_v1(uint64_t handle, connect_norito_kagemusha_wallet_result_v1* out);
int32_t connect_norito_kagemusha_wallet_credit_status_v1(uint64_t handle, const uint8_t* credit32, const uint8_t* payment32, connect_norito_kagemusha_wallet_result_v1* out);

/** Fixed Send/Unload Native intake: selector1/8; no foreign source identifiers. */
typedef struct connect_norito_kagemusha_wallet_review_request_v1 {
  uint32_t selector;
  connect_norito_kagemusha_wallet_u128_v1 amount;
  const uint8_t *first;
  size_t first_length;
  const uint8_t *second;
  size_t second_length;
} connect_norito_kagemusha_wallet_review_request_v1;
/** Authenticate only: separate result18, positive owner-local one-use token in sequence_low,
 * sequence_high/detail zero; exact491 DATA bytes KWORV1\0\0, operation1/8,
 * four LE UInt128 amount/fee/grossDebit/netDestination; receiverPresence byte+receiver32;
 * destinationAccount/request/charge/scheme/wallet/currentHead/sourceState/sourceCapsule/
 * credential/artifactManifest32 each, then canonical SEC1 paymentKey65. Copyable DATA
 * cannot reconstruct the actual retained Native review. Free with connect_norito_free. */
int32_t connect_norito_kagemusha_wallet_review_v1(uint64_t handle,
    const connect_norito_kagemusha_wallet_review_request_v1 *request,
    connect_norito_kagemusha_wallet_result_v1 *out);
/** Consume actual review after fresh hardware approval; current source and ordinary proofs
 * remain mandatory before Advance. No financial input is reconstructed from projection DATA. */
int32_t connect_norito_kagemusha_wallet_execute_reviewed_v1(uint64_t handle,
    uint64_t token, const uint8_t *request_id32, connect_norito_kagemusha_wallet_result_v1 *out);
/** Cancel an in-memory review only; no durable custody mutation. */
int32_t connect_norito_kagemusha_wallet_discard_review_v1(uint64_t handle, uint64_t token);

// Mandatory original base authenticated under public build-selected Ed25519 trust.
// Whole app manifest <=8MiB; envelope <=2048; wallet runtime <=128KiB;
// signed genesis <=64MiB; root UTF-8 <=4096. Verifier pack/catalog retain Native caps.
// All seven installation originals are required; absent input returns -1/no handle.
// Any nonempty financial offer must supply the complete genuine original graph.
typedef struct {
    const uint8_t* app_manifest; size_t app_manifest_length;
    const uint8_t* envelope; size_t envelope_length;
    const uint8_t* wallet_runtime; size_t wallet_runtime_length;
    const uint8_t* verifier_pack; size_t verifier_pack_length;
    const uint8_t* producer_inventory; size_t producer_inventory_length;
    const uint8_t* signed_genesis; size_t signed_genesis_length;
    const uint8_t* originals_root; size_t originals_root_length;
} connect_norito_kagemusha_wallet_runtime_originals_v1;
// Uses existing platform custody_root callback; retains actual provider in the existing
// Native runtime registry. Begin/finish/cancel/close remain the sole existing open APIs.
int32_t connect_norito_kagemusha_wallet_install_runtime_v1(
    const connect_norito_kagemusha_wallet_runtime_originals_v1* originals,
    const connect_norito_kagemusha_platform_v1* platform,
    uint64_t* out_runtime);

// Enrollment DATA in the existing installed runtime; no supplied freshness/profile flag.
typedef struct connect_norito_kagemusha_wallet_enrollment_request_v1 {
    uint32_t selector; // 0 begin/retry E1; 1 resume slot; 2 retain request; 3 store credential.
    const uint8_t* slot; size_t slot_length;
    const uint8_t* challenge; size_t challenge_length;
    const uint8_t* policy; size_t policy_length;
    const uint8_t* account; size_t account_length;
    const uint8_t* original; size_t original_length;
    const uint8_t* certificates; size_t certificates_length;
} connect_norito_kagemusha_wallet_enrollment_request_v1;
typedef struct connect_norito_kagemusha_wallet_enrollment_result_v1 {
    // 0 Enrolled; 1 Pending; 2 SlotAbandoned; 3 RequestRetained; 4 CredentialStored.
    // Negative values preserve the ordinary Native failure status, reason and OS code.
    int32_t status;
    int32_t reason;
    int32_t platform_code;
    uint8_t slot[32];
    uint8_t payment_key[65];
    size_t payment_key_length; // 65 only for Enrolled, zero otherwise.
    uint8_t* bytes; // Exact marker/request/credential for 0/3/4; connect_norito_free owns free.
    size_t length;
} connect_norito_kagemusha_wallet_enrollment_result_v1;
int32_t connect_norito_kagemusha_wallet_enrollment_v1(uint64_t runtime,
    const connect_norito_kagemusha_wallet_enrollment_request_v1* request,
    connect_norito_kagemusha_wallet_enrollment_result_v1* result);

#ifdef __cplusplus
} // extern "C"
#endif

#endif // CONNECT_NORITO_BRIDGE_H
