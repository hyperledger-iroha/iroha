package org.hyperledger.iroha.android.client;

import java.util.Objects;

/** Canonical payload covered by an identifier-resolution receipt attestation. */
public final class IdentifierResolutionPayload {
  private final String policyId;
  private final IdentifierResolutionExecutionPayload execution;
  private final RamLfeOutputOpening opening;
  private final String opaqueId;
  private final String receiptHash;
  private final String uaid;
  private final String accountId;

  public IdentifierResolutionPayload(
      final String policyId,
      final IdentifierResolutionExecutionPayload execution,
      final RamLfeOutputOpening opening,
      final String opaqueId,
      final String receiptHash,
      final String uaid,
      final String accountId) {
    this.policyId = Objects.requireNonNull(policyId, "policyId");
    this.execution = Objects.requireNonNull(execution, "execution");
    this.opening = Objects.requireNonNull(opening, "opening");
    this.opaqueId = Objects.requireNonNull(opaqueId, "opaqueId");
    this.receiptHash = Objects.requireNonNull(receiptHash, "receiptHash");
    this.uaid = Objects.requireNonNull(uaid, "uaid");
    this.accountId = Objects.requireNonNull(accountId, "accountId");
  }

  public String policyId() {
    return policyId;
  }

  public IdentifierResolutionExecutionPayload execution() {
    return execution;
  }

  public RamLfeOutputOpening opening() {
    return opening;
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
}
