package org.hyperledger.iroha.android.offline;

import java.util.Map;

/** Supplies device-bound proof material for Offline Note issuer mutations. */
public interface OfflineNoteIssuerDeviceProofProvider {
  Map<String, Object> currentDeviceProof(
      String chainId,
      String accountId,
      String assetDefinitionId,
      String operation,
      String lineageId,
      String amount);
}
