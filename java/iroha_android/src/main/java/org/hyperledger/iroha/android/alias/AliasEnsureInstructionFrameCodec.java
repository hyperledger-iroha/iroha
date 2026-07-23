package org.hyperledger.iroha.android.alias;

/** Registry hook that decodes and canonically re-encodes an EnsureAlias frame. */
@FunctionalInterface
public interface AliasEnsureInstructionFrameCodec {
  /** Decodes and re-encodes one exact planner frame. */
  DecodedEnsureAliasFrame decodeAndReencode(String wireId, byte[] framedPayload);
}
