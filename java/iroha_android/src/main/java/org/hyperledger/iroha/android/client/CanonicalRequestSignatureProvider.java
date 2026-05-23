package org.hyperledger.iroha.android.client;

/** Signs SDK-built canonical Torii request messages. */
@FunctionalInterface
public interface CanonicalRequestSignatureProvider {
  byte[] sign(byte[] message);
}
