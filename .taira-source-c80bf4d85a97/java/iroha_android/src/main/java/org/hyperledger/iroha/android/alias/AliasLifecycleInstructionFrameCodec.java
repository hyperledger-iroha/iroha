package org.hyperledger.iroha.android.alias;

/** Registry hook that round-trips a typed renewal or auto-renew instruction frame. */
@FunctionalInterface
public interface AliasLifecycleInstructionFrameCodec {
  /** Decodes and canonically re-encodes one exact lifecycle frame. */
  DecodedAliasLifecycleFrame decodeAndReencode(
      String wireId, byte[] framedPayload, int chainDiscriminant);
}
