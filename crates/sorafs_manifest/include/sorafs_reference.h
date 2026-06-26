// SoraFS reference validator C FFI header.
//
// This header is the contract consumed by non-Rust SDK bindings. Keep it in
// sync with crates/sorafs_manifest/src/reference_ffi.rs by running
// ci/check_sorafs_reference_ffi_header.sh after changing exported FFI symbols.

#ifndef SORAFS_REFERENCE_H
#define SORAFS_REFERENCE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define SORAFS_REFERENCE_REPAIR_KIND_EVIDENCE 1
#define SORAFS_REFERENCE_REPAIR_KIND_REPORT 2
#define SORAFS_REFERENCE_REPAIR_KIND_TASK_RECORD 3
#define SORAFS_REFERENCE_REPAIR_KIND_SLASH_PROPOSAL 4
#define SORAFS_REFERENCE_REPAIR_KIND_ESCALATION_POLICY 5
#define SORAFS_REFERENCE_REPAIR_KIND_ESCALATION_APPROVAL 6
#define SORAFS_REFERENCE_REPAIR_KIND_SIGNED_AUDITOR_REQUEST 7
#define SORAFS_REFERENCE_REPAIR_KIND_WORKER_SIGNATURE 8
#define SORAFS_REFERENCE_REPAIR_KIND_TASK_EVENT 9
#define SORAFS_REFERENCE_REPAIR_KIND_AUDIT_EVENT 10

#define SORAFS_REFERENCE_ORDERBOOK_KIND_ORDER_REQUEST 1
#define SORAFS_REFERENCE_ORDERBOOK_KIND_ORDER_CANCEL 2
#define SORAFS_REFERENCE_ORDERBOOK_KIND_TRADE_EVENT 3
#define SORAFS_REFERENCE_ORDERBOOK_KIND_SETTLEMENT_CHANNEL 4
#define SORAFS_REFERENCE_ORDERBOOK_KIND_SETTLEMENT_RECEIPT 5
#define SORAFS_REFERENCE_ORDERBOOK_KIND_RUNTIME_SNAPSHOT 6

#define SORAFS_REFERENCE_POP_KIND_CREDENTIAL 1
#define SORAFS_REFERENCE_POP_KIND_COMMITMENT_ROOT 2
#define SORAFS_REFERENCE_POP_KIND_REVOCATION_LIST 3
#define SORAFS_REFERENCE_POP_KIND_ENROLLMENT_REQUEST 4
#define SORAFS_REFERENCE_POP_KIND_RENEWAL_REQUEST 5
#define SORAFS_REFERENCE_POP_KIND_MEMBERSHIP_PROOF 6
#define SORAFS_REFERENCE_POP_KIND_ISSUED_CREDENTIAL_BUNDLE 7

#define SORAFS_REFERENCE_HEDGING_KIND_PRICE_FEED 1
#define SORAFS_REFERENCE_HEDGING_KIND_REFERENCE_PRICE_DECISION 2
#define SORAFS_REFERENCE_HEDGING_KIND_BILLING_LINE_ITEM 3
#define SORAFS_REFERENCE_HEDGING_KIND_BILLING_STATEMENT 4

#define SORAFS_REFERENCE_BUNDLE_KIND_PROVIDER_ADVERT 1
#define SORAFS_REFERENCE_BUNDLE_KIND_PROVIDER_ADMISSION_ENVELOPE 2
#define SORAFS_REFERENCE_BUNDLE_KIND_REPLICATION_ORDER 3
#define SORAFS_REFERENCE_BUNDLE_KIND_POR_CHALLENGE 4
#define SORAFS_REFERENCE_BUNDLE_KIND_POR_PROOF 5
#define SORAFS_REFERENCE_BUNDLE_KIND_POTR_RECEIPT 6
#define SORAFS_REFERENCE_BUNDLE_KIND_REPAIR_EVIDENCE 7
#define SORAFS_REFERENCE_BUNDLE_KIND_REPAIR_REPORT 8
#define SORAFS_REFERENCE_BUNDLE_KIND_REPAIR_TASK_RECORD 9
#define SORAFS_REFERENCE_BUNDLE_KIND_REPAIR_SLASH_PROPOSAL 10
#define SORAFS_REFERENCE_BUNDLE_KIND_REPAIR_TASK_EVENT 11
#define SORAFS_REFERENCE_BUNDLE_KIND_ORDERBOOK_ORDER_REQUEST 12
#define SORAFS_REFERENCE_BUNDLE_KIND_ORDERBOOK_ORDER_CANCEL 13
#define SORAFS_REFERENCE_BUNDLE_KIND_ORDERBOOK_TRADE_EVENT 14
#define SORAFS_REFERENCE_BUNDLE_KIND_ORDERBOOK_SETTLEMENT_CHANNEL 15
#define SORAFS_REFERENCE_BUNDLE_KIND_ORDERBOOK_SETTLEMENT_RECEIPT 16
#define SORAFS_REFERENCE_BUNDLE_KIND_PDP_COMMITMENT 17
#define SORAFS_REFERENCE_BUNDLE_KIND_PDP_CHALLENGE 18
#define SORAFS_REFERENCE_BUNDLE_KIND_PDP_PROOF 19
#define SORAFS_REFERENCE_BUNDLE_KIND_ORDERBOOK_RUNTIME_SNAPSHOT 20

#define SORAFS_REFERENCE_PROFILE_NONE 0
#define SORAFS_REFERENCE_PROFILE_HOT 1
#define SORAFS_REFERENCE_PROFILE_WARM 2
#define SORAFS_REFERENCE_PROFILE_ARCHIVE 3

typedef struct SorafsReferenceFfiBuffer {
  uint8_t *ptr;
  size_t len;
} SorafsReferenceFfiBuffer;

typedef struct SorafsReferenceFfiBundlePayload {
  uint32_t kind;
  const uint8_t *bytes_ptr;
  size_t bytes_len;
  const uint8_t *label_ptr;
  size_t label_len;
} SorafsReferenceFfiBundlePayload;

void sorafs_reference_free_buffer(SorafsReferenceFfiBuffer buffer);

SorafsReferenceFfiBuffer sorafs_reference_validate_provider_advert_json(
    const uint8_t *bytes_ptr, size_t bytes_len, const uint8_t *label_ptr,
    size_t label_len, uint64_t now, uint64_t generated_at);

SorafsReferenceFfiBuffer sorafs_reference_validate_provider_admission_json(
    const uint8_t *bytes_ptr, size_t bytes_len, const uint8_t *label_ptr,
    size_t label_len, uint64_t generated_at);

SorafsReferenceFfiBuffer
sorafs_reference_validate_provider_admission_renewal_json(
    const uint8_t *envelope_ptr, size_t envelope_len,
    const uint8_t *envelope_label_ptr, size_t envelope_label_len,
    const uint8_t *renewal_ptr, size_t renewal_len,
    const uint8_t *renewal_label_ptr, size_t renewal_label_len,
    uint64_t generated_at);

SorafsReferenceFfiBuffer
sorafs_reference_validate_provider_admission_revocation_json(
    const uint8_t *envelope_ptr, size_t envelope_len,
    const uint8_t *envelope_label_ptr, size_t envelope_label_len,
    const uint8_t *revocation_ptr, size_t revocation_len,
    const uint8_t *revocation_label_ptr, size_t revocation_label_len,
    uint64_t generated_at);

SorafsReferenceFfiBuffer sorafs_reference_validate_replication_order_json(
    const uint8_t *bytes_ptr, size_t bytes_len, const uint8_t *label_ptr,
    size_t label_len, uint64_t generated_at);

SorafsReferenceFfiBuffer
sorafs_reference_validate_signed_replication_order_json(
    const uint8_t *bytes_ptr, size_t bytes_len, const uint8_t *label_ptr,
    size_t label_len, uint64_t generated_at);

SorafsReferenceFfiBuffer sorafs_reference_validate_orderbook_json(
    uint32_t kind, const uint8_t *bytes_ptr, size_t bytes_len,
    const uint8_t *label_ptr, size_t label_len, uint64_t generated_at);

SorafsReferenceFfiBuffer sorafs_reference_validate_pop_json(
    uint32_t kind, const uint8_t *bytes_ptr, size_t bytes_len,
    const uint8_t *label_ptr, size_t label_len, uint64_t generated_at);

SorafsReferenceFfiBuffer sorafs_reference_validate_hedging_json(
    uint32_t kind, const uint8_t *bytes_ptr, size_t bytes_len,
    const uint8_t *label_ptr, size_t label_len, uint64_t generated_at);

SorafsReferenceFfiBuffer sorafs_reference_validate_pdp_commitment_json(
    const uint8_t *bytes_ptr, size_t bytes_len, const uint8_t *label_ptr,
    size_t label_len, uint64_t generated_at);

SorafsReferenceFfiBuffer sorafs_reference_validate_pdp_challenge_json(
    const uint8_t *bytes_ptr, size_t bytes_len, const uint8_t *label_ptr,
    size_t label_len, uint64_t generated_at);

SorafsReferenceFfiBuffer sorafs_reference_validate_pdp_proof_json(
    const uint8_t *bytes_ptr, size_t bytes_len, const uint8_t *label_ptr,
    size_t label_len, uint64_t generated_at);

SorafsReferenceFfiBuffer
sorafs_reference_validate_pdp_commitment_challenge_json(
    const uint8_t *commitment_ptr, size_t commitment_len,
    const uint8_t *commitment_label_ptr, size_t commitment_label_len,
    const uint8_t *challenge_ptr, size_t challenge_len,
    const uint8_t *challenge_label_ptr, size_t challenge_label_len,
    uint64_t generated_at);

SorafsReferenceFfiBuffer sorafs_reference_validate_pdp_challenge_proof_json(
    const uint8_t *challenge_ptr, size_t challenge_len,
    const uint8_t *challenge_label_ptr, size_t challenge_label_len,
    const uint8_t *proof_ptr, size_t proof_len, const uint8_t *proof_label_ptr,
    size_t proof_label_len, uint64_t generated_at);

SorafsReferenceFfiBuffer sorafs_reference_validate_pdp_json(
    const uint8_t *commitment_ptr, size_t commitment_len,
    const uint8_t *commitment_label_ptr, size_t commitment_label_len,
    const uint8_t *challenge_ptr, size_t challenge_len,
    const uint8_t *challenge_label_ptr, size_t challenge_label_len,
    const uint8_t *proof_ptr, size_t proof_len, const uint8_t *proof_label_ptr,
    size_t proof_label_len, uint64_t generated_at);

SorafsReferenceFfiBuffer sorafs_reference_validate_por_json(
    const uint8_t *challenge_ptr, size_t challenge_len,
    const uint8_t *challenge_label_ptr, size_t challenge_label_len,
    const uint8_t *proof_ptr, size_t proof_len, const uint8_t *proof_label_ptr,
    size_t proof_label_len, uint64_t generated_at);

SorafsReferenceFfiBuffer sorafs_reference_validate_potr_json(
    const uint8_t *bytes_ptr, size_t bytes_len, const uint8_t *label_ptr,
    size_t label_len, uint32_t profile, uint64_t generated_at);

SorafsReferenceFfiBuffer sorafs_reference_validate_repair_json(
    uint32_t kind, const uint8_t *bytes_ptr, size_t bytes_len,
    const uint8_t *label_ptr, size_t label_len, uint64_t generated_at);

SorafsReferenceFfiBuffer sorafs_reference_validate_governance_json(
    const uint8_t *bytes_ptr, size_t bytes_len, const uint8_t *label_ptr,
    size_t label_len, const uint8_t *expected_cid_ptr,
    size_t expected_cid_len, uint64_t generated_at);

SorafsReferenceFfiBuffer sorafs_reference_validate_bundle_json(
    const SorafsReferenceFfiBundlePayload *payloads_ptr, size_t payloads_len,
    uint64_t now, uint64_t generated_at);

#ifdef __cplusplus
}
#endif

#endif // SORAFS_REFERENCE_H
