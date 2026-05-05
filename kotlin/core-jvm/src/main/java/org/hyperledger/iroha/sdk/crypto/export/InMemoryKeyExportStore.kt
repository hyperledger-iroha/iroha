package org.hyperledger.iroha.sdk.crypto.export

import java.util.concurrent.ConcurrentHashMap

/** In-memory key export store suitable for tests and ephemeral sessions. */
class InMemoryKeyExportStore : KeyExportStore {

    private val entries: ConcurrentHashMap<String, String> = ConcurrentHashMap()

    override fun load(alias: String): String? {
        if (alias.isBlank()) return null
        return entries[alias]
    }

    override fun store(alias: String, bundleBase64: String) {
        if (alias.isBlank()) throw KeyExportException("alias must be provided")
        if (bundleBase64.isBlank()) throw KeyExportException("bundleBase64 must be provided")
        entries[alias] = bundleBase64
    }

    override fun delete(alias: String) {
        if (alias.isBlank()) return
        entries.remove(alias)
    }
}
