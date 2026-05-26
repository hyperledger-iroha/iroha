package org.hyperledger.iroha.android.offline;

import java.util.UUID;

/** UUID-backed identifier generator. */
public final class UuidOfflineNoteIdGenerator implements OfflineNoteIdGenerator {
  @Override
  public String nextId(final String prefix) {
    return prefix + "-" + UUID.randomUUID();
  }
}
