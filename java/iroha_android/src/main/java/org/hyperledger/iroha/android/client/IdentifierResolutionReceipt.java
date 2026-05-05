package org.hyperledger.iroha.android.client;

import java.util.Objects;

/** Resolution receipt returned by identifier resolve and claim-receipt endpoints. */
public final class IdentifierResolutionReceipt {
  private final IdentifierResolutionPayload payload;
  private final IdentifierReceiptAttestation attestation;

  public IdentifierResolutionReceipt(
      final IdentifierResolutionPayload payload,
      final IdentifierReceiptAttestation attestation) {
    this.payload = Objects.requireNonNull(payload, "payload");
    this.attestation = Objects.requireNonNull(attestation, "attestation");
  }

  public IdentifierResolutionPayload payload() {
    return payload;
  }

  public IdentifierReceiptAttestation attestation() {
    return attestation;
  }

  public String policyId() {
    return payload.policyId();
  }

  public String opaqueId() {
    return payload.opaqueId();
  }

  public String receiptHash() {
    return payload.receiptHash();
  }

  public String uaid() {
    return payload.uaid();
  }

  public String accountId() {
    return payload.accountId();
  }

  public long resolvedAtMs() {
    return payload.execution().executedAtMs();
  }

  public Long expiresAtMs() {
    return payload.execution().expiresAtMs();
  }

  public String backend() {
    return payload.execution().backend();
  }

  public boolean verifyAttestation(final IdentifierPolicySummary policy) {
    return IdentifierReceiptVerifier.verify(this, policy);
  }
}
