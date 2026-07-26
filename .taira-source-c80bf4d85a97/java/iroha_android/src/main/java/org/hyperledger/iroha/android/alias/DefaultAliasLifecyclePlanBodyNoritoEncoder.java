package org.hyperledger.iroha.android.alias;

/** Default canonical lifecycle-plan body encoder. */
public enum DefaultAliasLifecyclePlanBodyNoritoEncoder
    implements AliasLifecyclePlanBodyNoritoEncoder {
  /** Shared stateless encoder. */
  INSTANCE;

  @Override
  public byte[] encode(final AliasLifecycleTransactionPlanBodyV1 body) {
    return AliasNoritoCodec.encodeLifecyclePlanBody(body);
  }
}
