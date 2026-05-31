package org.hyperledger.iroha.android.offline;

import java.util.List;
import java.util.concurrent.CompletableFuture;

/** Supplies recent Offline Note explorer outcomes for resolver-backed wallet sync. */
public interface OfflineNoteOutcomeProvider {
  CompletableFuture<List<OfflineNoteExplorerInstructionOutcome>> listOutcomes();
}
