package org.hyperledger.iroha.android.client;

import java.util.Objects;

/** Canonical payload covered by an identifier-resolution receipt signature. */
public final class IdentifierResolutionPayload {
  private final String policyId;
  private final String opaqueId;
  private final String receiptHash;
  private final String uaid;
  private final String accountId;
  private final long resolvedAtMs;
  private final Long expiresAtMs;

  public IdentifierResolutionPayload(
      final String policyId,
      final String opaqueId,
      final String receiptHash,
      final String uaid,
      final String accountId,
      final long resolvedAtMs,
      final Long expiresAtMs) {
    this.policyId = Objects.requireNonNull(policyId, "policyId");
    this.opaqueId = Objects.requireNonNull(opaqueId, "opaqueId");
    this.receiptHash = Objects.requireNonNull(receiptHash, "receiptHash");
    this.uaid = Objects.requireNonNull(uaid, "uaid");
    this.accountId = Objects.requireNonNull(accountId, "accountId");
    this.resolvedAtMs = resolvedAtMs;
    this.expiresAtMs = expiresAtMs;
  }

  public String policyId() {
    return policyId;
  }

  public String opaqueId() {
    return opaqueId;
  }

  public String receiptHash() {
    return receiptHash;
  }

  public String uaid() {
    return uaid;
  }

  public String accountId() {
    return accountId;
  }

  public long resolvedAtMs() {
    return resolvedAtMs;
  }

  public Long expiresAtMs() {
    return expiresAtMs;
  }
}
