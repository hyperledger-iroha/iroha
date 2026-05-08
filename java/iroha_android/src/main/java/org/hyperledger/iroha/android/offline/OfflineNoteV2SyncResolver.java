package org.hyperledger.iroha.android.offline;

import java.util.concurrent.CompletableFuture;

/** Looks up transaction-outcome state for pending wallet notes. */
public interface OfflineNoteV2SyncResolver {
  CompletableFuture<OfflineNoteV2SyncResolution> resolvePendingNote(OfflineNoteV2WalletNote note);
}
