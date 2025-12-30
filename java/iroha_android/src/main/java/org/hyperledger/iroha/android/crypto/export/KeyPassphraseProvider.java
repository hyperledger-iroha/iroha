package org.hyperledger.iroha.android.crypto.export;

/**
 * Supplies passphrases for deterministic key export/import flows.
 *
 * <p>Implementations should return a fresh char array per invocation so callers can safely zero
 * the returned buffer after use.
 */
@FunctionalInterface
public interface KeyPassphraseProvider {
  /** Returns the current passphrase as a mutable character array. */
  char[] passphrase();
}
