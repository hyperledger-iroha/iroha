package org.hyperledger.iroha.android.alias;

/** Default typed renewal and auto-renew registry codec. */
public enum DefaultAliasLifecycleInstructionFrameCodec
    implements AliasLifecycleInstructionFrameCodec {
  /** Shared stateless codec. */
  INSTANCE;

  @Override
  public DecodedAliasLifecycleFrame decodeAndReencode(
      final String wireId, final byte[] framedPayload) {
    if (RenewAliasLease.WIRE_ID.equals(wireId)) {
      final RenewAliasLease value = AliasNoritoCodec.decodeRenewAliasLeaseFrame(framedPayload);
      return new DecodedAliasLifecycleFrame(
          new AliasLifecycleOperationV1.RenewLease(value),
          AliasNoritoCodec.encodeRenewAliasLeaseFrame(value));
    }
    if (ConfigureAliasAutoRenew.WIRE_ID.equals(wireId)) {
      final ConfigureAliasAutoRenew value =
          AliasNoritoCodec.decodeConfigureAutoRenewFrame(framedPayload);
      return new DecodedAliasLifecycleFrame(
          new AliasLifecycleOperationV1.ConfigureAutoRenew(value),
          AliasNoritoCodec.encodeConfigureAutoRenewFrame(value));
    }
    throw new IllegalArgumentException("unsupported alias lifecycle wire id: " + wireId);
  }
}
