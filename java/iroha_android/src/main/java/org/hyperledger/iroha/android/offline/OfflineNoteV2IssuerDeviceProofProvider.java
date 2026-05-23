package org.hyperledger.iroha.android.offline;

import java.util.Map;

/** Supplies device-bound proof material for Offline Note V2 issuer mutations. */
public interface OfflineNoteV2IssuerDeviceProofProvider {
  Map<String, Object> currentDeviceProof(
      String chainId,
      String accountId,
      String assetDefinitionId,
      String operation,
      String lineageId,
      String amount);
}
