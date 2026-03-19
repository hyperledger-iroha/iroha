package org.hyperledger.iroha.android.client;

import java.util.Objects;

/** Persisted identifier-claim binding returned by receipt-hash lookup. */
public final class IdentifierClaimRecord {
  private final String policyId;
  private final String opaqueId;
  private final String receiptHash;
  private final String uaid;
  private final String accountId;
  private final long verifiedAtMs;
  private final Long expiresAtMs;

  public IdentifierClaimRecord(
      final String policyId,
      final String opaqueId,
      final String receiptHash,
      final String uaid,
      final String accountId,
      final long verifiedAtMs,
      final Long expiresAtMs) {
    this.policyId = Objects.requireNonNull(policyId, "policyId");
    this.opaqueId = Objects.requireNonNull(opaqueId, "opaqueId");
    this.receiptHash = Objects.requireNonNull(receiptHash, "receiptHash");
    this.uaid = Objects.requireNonNull(uaid, "uaid");
    this.accountId = Objects.requireNonNull(accountId, "accountId");
    this.verifiedAtMs = verifiedAtMs;
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

  public long verifiedAtMs() {
    return verifiedAtMs;
  }

  public Long expiresAtMs() {
    return expiresAtMs;
  }
}
