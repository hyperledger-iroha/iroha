package org.hyperledger.iroha.android.client;

import java.security.PrivateKey;
import java.util.Objects;

/** Canonical request signing material for Torii app endpoints. */
public final class ToriiCanonicalRequestAuth {

  private final String accountId;
  private final PrivateKey privateKey;
  private final Long timestampMs;
  private final String nonce;

  public ToriiCanonicalRequestAuth(final String accountId, final PrivateKey privateKey) {
    this(accountId, privateKey, null, null);
  }

  public ToriiCanonicalRequestAuth(
      final String accountId,
      final PrivateKey privateKey,
      final Long timestampMs,
      final String nonce) {
    this.accountId = Objects.requireNonNull(accountId, "accountId");
    this.privateKey = Objects.requireNonNull(privateKey, "privateKey");
    this.timestampMs = timestampMs;
    this.nonce = nonce;
  }

  public String accountId() {
    return accountId;
  }

  public PrivateKey privateKey() {
    return privateKey;
  }

  public Long timestampMs() {
    return timestampMs;
  }

  public String nonce() {
    return nonce;
  }
}
