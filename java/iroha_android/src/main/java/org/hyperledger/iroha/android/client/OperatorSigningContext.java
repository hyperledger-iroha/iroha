package org.hyperledger.iroha.android.client;

import java.util.Objects;
import org.hyperledger.iroha.android.model.NetworkId;

/** Immutable exact-network signing context for operator-only Torii APIs. */
public final class OperatorSigningContext {
  private final NetworkId networkId;
  private final String publicKey;
  private final OperatorRequestSignatureProvider signatureProvider;

  public OperatorSigningContext(
      final NetworkId networkId,
      final String publicKey,
      final OperatorRequestSignatureProvider signatureProvider) {
    this.networkId = Objects.requireNonNull(networkId, "networkId");
    this.publicKey = requirePublicKey(publicKey);
    this.signatureProvider = Objects.requireNonNull(signatureProvider, "signatureProvider");
  }

  /** Returns the exact genesis-derived NetworkId included in every signature. */
  public NetworkId networkId() {
    return networkId;
  }

  /** Returns the canonical public-key multihash sent with each signature. */
  public String publicKey() {
    return publicKey;
  }

  byte[] sign(final byte[] message) {
    final byte[] signature = signatureProvider.sign(message.clone());
    if (signature == null || signature.length == 0) {
      throw new IllegalArgumentException("operator signer returned an empty signature");
    }
    return signature.clone();
  }

  private static String requirePublicKey(final String value) {
    if (value == null
        || value.isEmpty()
        || value.length() > 512
        || !value.equals(value.trim())) {
      throw new IllegalArgumentException(
          "operator publicKey must be exact non-empty printable ASCII");
    }
    for (int index = 0; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (character < 0x21 || character > 0x7e) {
        throw new IllegalArgumentException(
            "operator publicKey must be exact non-empty printable ASCII");
      }
    }
    return value;
  }
}
