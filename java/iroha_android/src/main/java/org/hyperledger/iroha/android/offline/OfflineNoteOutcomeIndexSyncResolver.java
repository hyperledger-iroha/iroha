package org.hyperledger.iroha.android.offline;

import java.util.Objects;
import java.util.concurrent.CompletableFuture;

/** Sync resolver that rebuilds an outcome index from a provider for each wallet sync pass. */
public final class OfflineNoteOutcomeIndexSyncResolver implements OfflineNoteSyncResolver {
  private final OfflineNoteOutcomeProvider provider;

  public OfflineNoteOutcomeIndexSyncResolver(final OfflineNoteOutcomeProvider provider) {
    this.provider = Objects.requireNonNull(provider, "provider");
  }

  @Override
  public CompletableFuture<OfflineNoteSyncResolution> resolvePendingNote(
      final OfflineNoteWalletNote note) {
    return provider
        .listOutcomes()
        .thenApply(outcomes -> OfflineNoteOutcomeIndex.fromExplorerOutcomes(outcomes).resolve(note));
  }
}
