package org.hyperledger.iroha.android.alias;

/** Default canonical setup-plan body encoder. */
public enum DefaultAliasPlanBodyNoritoEncoder implements AliasPlanBodyNoritoEncoder {
  /** Shared stateless encoder. */
  INSTANCE;

  @Override
  public byte[] encode(final AliasSetupModels.AliasTransactionPlanBodyV1 body) {
    return AliasNoritoCodec.encodePlanBody(body);
  }
}
