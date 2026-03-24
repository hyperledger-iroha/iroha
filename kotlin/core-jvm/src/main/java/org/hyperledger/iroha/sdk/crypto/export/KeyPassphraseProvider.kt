package org.hyperledger.iroha.sdk.crypto.export

/**
 * Supplies passphrases for deterministic key export/import flows.
 *
 * Implementations should return a fresh char array per invocation so callers can safely zero
 * the returned buffer after use.
 */
fun interface KeyPassphraseProvider {
    /** Returns the current passphrase as a mutable character array. */
    fun passphrase(): CharArray
}
