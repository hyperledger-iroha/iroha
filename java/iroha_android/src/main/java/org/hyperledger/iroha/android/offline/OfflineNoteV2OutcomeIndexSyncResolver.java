package org.hyperledger.iroha.android.offline;

import java.util.Objects;
import java.util.concurrent.CompletableFuture;

/** Sync resolver that rebuilds an outcome index from a provider for each wallet sync pass. */
public final class OfflineNoteV2OutcomeIndexSyncResolver implements OfflineNoteV2SyncResolver {
  private final OfflineNoteV2OutcomeProvider provider;

  public OfflineNoteV2OutcomeIndexSyncResolver(final OfflineNoteV2OutcomeProvider provider) {
    this.provider = Objects.requireNonNull(provider, "provider");
  }

  @Override
  public CompletableFuture<OfflineNoteV2SyncResolution> resolvePendingNote(
      final OfflineNoteV2WalletNote note) {
    return provider
        .listOutcomes()
        .thenApply(outcomes -> OfflineNoteV2OutcomeIndex.fromExplorerOutcomes(outcomes).resolve(note));
  }
}
