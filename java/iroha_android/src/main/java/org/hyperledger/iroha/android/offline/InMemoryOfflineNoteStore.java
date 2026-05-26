package org.hyperledger.iroha.android.offline;

import java.util.LinkedHashMap;
import java.util.Map;

/** In-memory store for Java tests and non-persistent tooling. */
public final class InMemoryOfflineNoteStore implements OfflineNoteStore {
  private final Map<String, OfflineNoteWalletNote> notes = new LinkedHashMap<>();

  @Override
  public synchronized <T> T mutateNotes(final Mutation<T> mutation) {
    return mutation.apply(notes);
  }
}
