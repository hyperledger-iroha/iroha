package org.hyperledger.iroha.android.alias;

/** Default typed EnsureAlias registry codec. */
public enum DefaultAliasEnsureInstructionFrameCodec
    implements AliasEnsureInstructionFrameCodec {
  /** Shared stateless codec. */
  INSTANCE;

  @Override
  public DecodedEnsureAliasFrame decodeAndReencode(
      final String wireId, final byte[] framedPayload) {
    if (!EnsureAlias.WIRE_ID.equals(wireId)) {
      throw new IllegalArgumentException("unsupported alias setup wire id: " + wireId);
    }
    final EnsureAlias value = AliasNoritoCodec.decodeEnsureAliasFrame(framedPayload);
    return new DecodedEnsureAliasFrame(value, AliasNoritoCodec.encodeEnsureAliasFrame(value));
  }
}
