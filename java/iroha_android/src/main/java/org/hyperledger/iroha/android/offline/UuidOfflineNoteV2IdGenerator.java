package org.hyperledger.iroha.android.offline;

import java.util.UUID;

/** UUID-backed identifier generator. */
public final class UuidOfflineNoteV2IdGenerator implements OfflineNoteV2IdGenerator {
  @Override
  public String nextId(final String prefix) {
    return prefix + "-" + UUID.randomUUID();
  }
}
