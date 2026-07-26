package org.hyperledger.iroha.android.crypto.export;

import java.util.Optional;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ConcurrentMap;

/** In-memory key export store suitable for tests and ephemeral sessions. */
public final class InMemoryKeyExportStore implements KeyExportStore {
  private final ConcurrentMap<String, String> entries = new ConcurrentHashMap<>();

  @Override
  public Optional<String> load(final String alias) {
    if (alias == null || alias.trim().isEmpty()) {
      return Optional.empty();
    }
    return Optional.ofNullable(entries.get(alias));
  }

  @Override
  public void store(final String alias, final String bundleBase64) throws KeyExportException {
    if (alias == null || alias.trim().isEmpty()) {
      throw new KeyExportException("alias must be provided");
    }
    if (bundleBase64 == null || bundleBase64.trim().isEmpty()) {
      throw new KeyExportException("bundleBase64 must be provided");
    }
    entries.put(alias, bundleBase64);
  }

  @Override
  public void delete(final String alias) {
    if (alias == null || alias.trim().isEmpty()) {
      return;
    }
    entries.remove(alias);
  }
}
