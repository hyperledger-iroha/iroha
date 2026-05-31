package org.hyperledger.iroha.android.offline;

import java.util.concurrent.CompletableFuture;

/** Looks up transaction-outcome state for pending wallet notes. */
public interface OfflineNoteSyncResolver {
  CompletableFuture<OfflineNoteSyncResolution> resolvePendingNote(OfflineNoteWalletNote note);
}
