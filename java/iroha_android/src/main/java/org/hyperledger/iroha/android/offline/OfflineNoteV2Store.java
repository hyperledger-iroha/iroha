package org.hyperledger.iroha.android.offline;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/** Minimal structured store API for Offline Note V2 wallet notes. */
public interface OfflineNoteV2Store {
  interface Mutation<T> {
    T apply(Map<String, OfflineNoteV2WalletNote> notes);
  }

  <T> T mutateNotes(Mutation<T> mutation);

  default List<OfflineNoteV2WalletNote> listNotes() {
    return mutateNotes(notes -> new ArrayList<>(notes.values()));
  }

  default OfflineNoteV2WalletNote findNote(final byte[] noteCommitment) {
    return mutateNotes(notes -> notes.get(OfflineNoteV2Wallet.hexLower(noteCommitment)));
  }

  default void upsert(final OfflineNoteV2WalletNote note) {
    mutateNotes(notes -> {
      notes.put(note.noteCommitmentHex(), note);
      return null;
    });
  }
}
