package org.hyperledger.iroha.android.client;

import java.util.Objects;

/** Resolution receipt returned by identifier resolve and claim-receipt endpoints. */
public final class IdentifierResolutionReceipt {
  private final String policyId;
  private final String opaqueId;
  private final String receiptHash;
  private final String uaid;
  private final String accountId;
  private final long resolvedAtMs;
  private final Long expiresAtMs;
  private final String backend;
  private final String signature;
  private final String signaturePayloadHex;
  private final IdentifierResolutionPayload signaturePayload;

  public IdentifierResolutionReceipt(
      final String policyId,
      final String opaqueId,
      final String receiptHash,
      final String uaid,
      final String accountId,
      final long resolvedAtMs,
      final Long expiresAtMs,
      final String backend,
      final String signature,
      final String signaturePayloadHex,
      final IdentifierResolutionPayload signaturePayload) {
    this.policyId = Objects.requireNonNull(policyId, "policyId");
    this.opaqueId = Objects.requireNonNull(opaqueId, "opaqueId");
    this.receiptHash = Objects.requireNonNull(receiptHash, "receiptHash");
    this.uaid = Objects.requireNonNull(uaid, "uaid");
    this.accountId = Objects.requireNonNull(accountId, "accountId");
    this.resolvedAtMs = resolvedAtMs;
    this.expiresAtMs = expiresAtMs;
    this.backend = Objects.requireNonNull(backend, "backend");
    this.signature = Objects.requireNonNull(signature, "signature");
    this.signaturePayloadHex = Objects.requireNonNull(signaturePayloadHex, "signaturePayloadHex");
    this.signaturePayload = Objects.requireNonNull(signaturePayload, "signaturePayload");
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

  public String backend() {
    return backend;
  }

  public String signature() {
    return signature;
  }

  public String signaturePayloadHex() {
    return signaturePayloadHex;
  }

  public IdentifierResolutionPayload signaturePayload() {
    return signaturePayload;
  }

  public boolean verifySignature(final IdentifierPolicySummary policy) {
    return IdentifierReceiptVerifier.verify(this, policy);
  }
}
