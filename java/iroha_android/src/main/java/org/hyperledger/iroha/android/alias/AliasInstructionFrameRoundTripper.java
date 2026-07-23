package org.hyperledger.iroha.android.alias;

/** Decodes and re-encodes a framed instruction using the SDK instruction registry. */
@FunctionalInterface
public interface AliasInstructionFrameRoundTripper {
  /** Returns the re-encoded frame for the supplied stable wire identifier and exact payload. */
  byte[] decodeAndReencode(String wireId, byte[] framedPayload);
}

