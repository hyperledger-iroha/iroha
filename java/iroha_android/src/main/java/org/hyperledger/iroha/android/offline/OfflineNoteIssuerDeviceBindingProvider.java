package org.hyperledger.iroha.android.offline;

/** Supplies the current issuer device binding and attestation receipt. */
public interface OfflineNoteIssuerDeviceBindingProvider {
  OfflineNoteIssuerDeviceBinding currentDeviceBinding(
      String chainId, String accountId, String assetDefinitionId);
}
