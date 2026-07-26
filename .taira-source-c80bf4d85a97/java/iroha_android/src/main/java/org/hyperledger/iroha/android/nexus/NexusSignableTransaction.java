package org.hyperledger.iroha.android.nexus;

import java.util.Objects;

/** Canonical transaction payload to be signed by a wallet. */
public final class NexusSignableTransaction {

  private final byte[] payloadBytes;
  private final String payloadHashHex;
  private final String authority;
  private final byte[] signingPublicKey;
  private final String signatureAlgorithm;

  public NexusSignableTransaction(
      final byte[] payloadBytes,
      final String payloadHashHex,
      final String authority,
      final byte[] signingPublicKey,
      final String signatureAlgorithm) {
    this.payloadBytes = NexusModelUtils.copy(Objects.requireNonNull(payloadBytes, "payloadBytes"));
    this.payloadHashHex = NexusModelUtils.requireNonBlank(payloadHashHex, "payloadHashHex");
    this.authority = NexusModelUtils.requireNonBlank(authority, "authority");
    this.signingPublicKey = NexusModelUtils.copy(Objects.requireNonNull(signingPublicKey, "signingPublicKey"));
    this.signatureAlgorithm = NexusModelUtils.requireNonBlank(signatureAlgorithm, "signatureAlgorithm");
  }

  public byte[] payloadBytes() {
    return NexusModelUtils.copy(payloadBytes);
  }

  public String payloadHashHex() {
    return payloadHashHex;
  }

  public String authority() {
    return authority;
  }

  public byte[] signingPublicKey() {
    return NexusModelUtils.copy(signingPublicKey);
  }

  public String signatureAlgorithm() {
    return signatureAlgorithm;
  }
}
