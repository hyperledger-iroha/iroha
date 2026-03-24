package org.hyperledger.iroha.sdk.tx.offline

/** Configuration for building `OfflineSigningEnvelope` instances. */
class OfflineEnvelopeOptions(
    @JvmField val issuedAtMs: Long = System.currentTimeMillis(),
    metadata: Map<String, String> = emptyMap(),
    exportedKeyBundle: ByteArray? = null,
) {
    private val _metadata: Map<String, String> = metadata.toMap()
    private val _exportedKeyBundle: ByteArray? = exportedKeyBundle?.copyOf()

    init {
        require(issuedAtMs >= 0) { "issuedAtMs must be non-negative" }
    }

    val metadata: Map<String, String> get() = _metadata

    val exportedKeyBundle: ByteArray? get() = _exportedKeyBundle?.copyOf()
}
