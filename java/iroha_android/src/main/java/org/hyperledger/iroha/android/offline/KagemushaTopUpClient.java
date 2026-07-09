package org.hyperledger.iroha.android.offline;

import java.util.concurrent.CompletableFuture;

/** Adapter boundary for the Torii Kagemusha online-to-offline top-up route. */
public interface KagemushaTopUpClient {
  CompletableFuture<KagemushaTopUpResponse> submitKagemushaTopUp(
      String chainId, String accountId, String assetDefinitionId, byte[] topUpRequestArchive);
}
