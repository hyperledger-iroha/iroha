package org.hyperledger.iroha.android.client;

/**
 * Opaque signer for a native-built Iroha query.
 *
 * <p>The callback receives only the exact 32-byte digest produced by the ABI-22 native codec.
 * Implementations should delegate to Android Keystore/KeyMint or another non-exportable signing
 * handle; private-key bytes are neither accepted nor returned by this interface.
 */
@FunctionalInterface
public interface IrohaQuerySignatureProvider {
  /**
   * Returns the canonical raw Iroha signature payload over {@code nativeQueryDigest}.
   *
   * <p>For example, Ed25519 is 64 raw bytes and secp256k1 is low-S {@code r || s}, not DER. The
   * native finalizer derives the algorithm from the authority and rejects every other shape.
   */
  byte[] signQueryDigest(byte[] nativeQueryDigest);
}
