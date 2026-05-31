package org.hyperledger.iroha.android.offline;

import java.util.concurrent.CompletableFuture;

/** Adapter boundary for Torii issuer key refill and note issue calls. */
public interface OfflineNoteIssuerClient {
  CompletableFuture<OfflineNoteLoadContext> prepareLoad(
      String chainId, String accountId, String assetDefinitionId, String amount);
  CompletableFuture<OfflineNoteIssueResponse> issueNote(OfflineNoteIssueRequest request);
}
