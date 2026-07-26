package org.hyperledger.iroha.android.crypto.export;

import java.util.Optional;

/**
 * Persistence backend for deterministic key export bundles.
 *
 * <p>Stores the base64-encoded representation returned by {@link KeyExportBundle#encodeBase64()}.
 */
public interface KeyExportStore {
  /** Loads the stored bundle for {@code alias}, when present. */
  Optional<String> load(String alias) throws KeyExportException;

  /** Stores the base64-encoded bundle for {@code alias}. */
  void store(String alias, String bundleBase64) throws KeyExportException;

  /** Removes any stored bundle for {@code alias}. */
  void delete(String alias) throws KeyExportException;
}
