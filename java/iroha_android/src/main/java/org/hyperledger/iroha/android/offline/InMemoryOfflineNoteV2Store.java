package org.hyperledger.iroha.android.offline;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** In-memory store for Java tests and non-persistent tooling. */
public final class InMemoryOfflineNoteV2Store implements OfflineNoteV2Store {
  private final Map<String, OfflineNoteV2WalletNote> notes = new LinkedHashMap<>();

  @Override
  public synchronized List<OfflineNoteV2WalletNote> listNotes() {
    return new ArrayList<>(notes.values());
  }

  @Override
  public synchronized OfflineNoteV2WalletNote findNote(final byte[] noteCommitment) {
    return notes.get(OfflineNoteV2Wallet.hexLower(noteCommitment));
  }

  @Override
  public synchronized void upsert(final OfflineNoteV2WalletNote note) {
    notes.put(note.noteCommitmentHex(), note);
  }
}
