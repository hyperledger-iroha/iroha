package org.hyperledger.iroha.android.client;

import java.util.Objects;

/** Canonical request signing material for Torii app endpoints. */
public final class ToriiCanonicalRequestAuth {

  private final String accountId;
  private final CanonicalRequestSignatureProvider signatureProvider;
  private final Long timestampMs;
  private final String nonce;

  public ToriiCanonicalRequestAuth(
      final String accountId, final CanonicalRequestSignatureProvider signatureProvider) {
    this(accountId, signatureProvider, null, null);
  }

  public ToriiCanonicalRequestAuth(
      final String accountId,
      final CanonicalRequestSignatureProvider signatureProvider,
      final Long timestampMs,
      final String nonce) {
    this.accountId = Objects.requireNonNull(accountId, "accountId");
    this.signatureProvider = Objects.requireNonNull(signatureProvider, "signatureProvider");
    this.timestampMs = timestampMs;
    this.nonce = nonce;
  }

  public String accountId() {
    return accountId;
  }

  public byte[] sign(final byte[] message) {
    final byte[] signature = signatureProvider.sign(Objects.requireNonNull(message, "message"));
    if (signature == null || signature.length == 0) {
      throw new IllegalStateException("canonical request signature is empty");
    }
    return signature;
  }

  public Long timestampMs() {
    return timestampMs;
  }

  public String nonce() {
    return nonce;
  }
}
