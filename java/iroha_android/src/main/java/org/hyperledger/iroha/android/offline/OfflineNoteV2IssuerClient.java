package org.hyperledger.iroha.android.offline;

import java.util.concurrent.CompletableFuture;

/** Adapter boundary for Torii issuer key refill and note issue calls. */
public interface OfflineNoteV2IssuerClient {
  CompletableFuture<OfflineNoteV2LoadContext> prepareLoad(
      String chainId, String accountId, String assetDefinitionId, String amount);
  CompletableFuture<OfflineNoteV2IssueResponse> issueNote(OfflineNoteV2IssueRequest request);
}
