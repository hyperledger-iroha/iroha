package org.hyperledger.iroha.android.offline;

import java.util.Arrays;

/** Immutable view over a pending offline journal entry. */
public final class OfflineJournalEntry {

  private final byte[] txId;
  private final byte[] payload;
  private final long recordedAtMs;
  private final byte[] hashChain;

  OfflineJournalEntry(
      final byte[] txId, final byte[] payload, final long recordedAtMs, final byte[] hashChain) {
    this.txId = Arrays.copyOf(txId, txId.length);
    this.payload = Arrays.copyOf(payload, payload.length);
    this.recordedAtMs = recordedAtMs;
    this.hashChain = Arrays.copyOf(hashChain, hashChain.length);
  }

  public byte[] txId() {
    return Arrays.copyOf(txId, txId.length);
  }

  public byte[] payload() {
    return Arrays.copyOf(payload, payload.length);
  }

  public long recordedAtMs() {
    return recordedAtMs;
  }

  public byte[] hashChain() {
    return Arrays.copyOf(hashChain, hashChain.length);
  }
}
