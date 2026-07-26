package org.hyperledger.iroha.android.nexus;

import java.util.Objects;

/** Wallet signature over {@link NexusSignableTransaction#payloadBytes()}. */
public final class NexusWalletSignature {

  private final byte[] signature;
  private final String algorithm;

  public NexusWalletSignature(final byte[] signature) {
    this(signature, NexusAppClient.SIGNATURE_ALGORITHM_ED25519);
  }

  public NexusWalletSignature(final byte[] signature, final String algorithm) {
    this.signature = NexusModelUtils.copy(Objects.requireNonNull(signature, "signature"));
    this.algorithm = NexusModelUtils.requireNonBlank(algorithm, "algorithm");
  }

  public byte[] signature() {
    return NexusModelUtils.copy(signature);
  }

  public String algorithm() {
    return algorithm;
  }
}
