package org.hyperledger.iroha.sdk.crypto.export

/**
 * Persistence backend for deterministic key export bundles.
 *
 * Stores the base64-encoded representation returned by `KeyExportBundle.encodeBase64()`.
 */
interface KeyExportStore {
    /** Loads the stored bundle for `alias`, when present. */
    @Throws(KeyExportException::class)
    fun load(alias: String): String?

    /** Stores the base64-encoded bundle for `alias`. */
    @Throws(KeyExportException::class)
    fun store(alias: String, bundleBase64: String)

    /** Removes any stored bundle for `alias`. */
    @Throws(KeyExportException::class)
    fun delete(alias: String)
}
