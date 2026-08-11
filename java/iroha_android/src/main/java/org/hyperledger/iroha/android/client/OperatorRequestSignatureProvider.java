package org.hyperledger.iroha.android.client;

/** Signs one SDK-built exact-network operator request message. */
@FunctionalInterface
public interface OperatorRequestSignatureProvider {
  /** Returns detached signature bytes for {@code message}. */
  byte[] sign(byte[] message);
}
