package org.hyperledger.iroha.android.offline;

/** Supplies the current issuer device binding and attestation receipt. */
public interface OfflineNoteV2IssuerDeviceBindingProvider {
  OfflineNoteV2IssuerDeviceBinding currentDeviceBinding(
      String chainId, String accountId, String assetDefinitionId);
}
