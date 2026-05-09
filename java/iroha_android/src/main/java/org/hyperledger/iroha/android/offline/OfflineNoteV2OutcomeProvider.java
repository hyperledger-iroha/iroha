package org.hyperledger.iroha.android.offline;

import java.util.List;
import java.util.concurrent.CompletableFuture;

/** Supplies recent Offline Note V2 explorer outcomes for resolver-backed wallet sync. */
public interface OfflineNoteV2OutcomeProvider {
  CompletableFuture<List<OfflineNoteV2ExplorerInstructionOutcome>> listOutcomes();
}
