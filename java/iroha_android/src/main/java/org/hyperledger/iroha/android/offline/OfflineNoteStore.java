package org.hyperledger.iroha.android.offline;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/** Minimal structured store API for Offline Note wallet notes. */
public interface OfflineNoteStore {
  interface Mutation<T> {
    T apply(Map<String, OfflineNoteWalletNote> notes);
  }

  <T> T mutateNotes(Mutation<T> mutation);

  default List<OfflineNoteWalletNote> listNotes() {
    return mutateNotes(notes -> new ArrayList<>(notes.values()));
  }

  default OfflineNoteWalletNote findNote(final byte[] noteCommitment) {
    return mutateNotes(notes -> notes.get(OfflineNoteWallet.hexLower(noteCommitment)));
  }

  default void upsert(final OfflineNoteWalletNote note) {
    mutateNotes(notes -> {
      notes.put(note.noteCommitmentHex(), note);
      return null;
    });
  }
}
