package org.hyperledger.iroha.android.offline;

import java.util.LinkedHashMap;
import java.util.Map;

/** In-memory store for Java tests and non-persistent tooling. */
public final class InMemoryOfflineNoteV2Store implements OfflineNoteV2Store {
  private final Map<String, OfflineNoteV2WalletNote> notes = new LinkedHashMap<>();

  @Override
  public synchronized <T> T mutateNotes(final Mutation<T> mutation) {
    return mutation.apply(notes);
  }
}
