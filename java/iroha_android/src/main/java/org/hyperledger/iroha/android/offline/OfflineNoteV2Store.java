package org.hyperledger.iroha.android.offline;

import java.util.List;

/** Minimal structured store API for Offline Note V2 wallet notes. */
public interface OfflineNoteV2Store {
  List<OfflineNoteV2WalletNote> listNotes();
  OfflineNoteV2WalletNote findNote(byte[] noteCommitment);
  void upsert(OfflineNoteV2WalletNote note);
}
