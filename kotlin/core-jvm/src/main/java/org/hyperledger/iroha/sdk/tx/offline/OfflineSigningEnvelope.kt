package org.hyperledger.iroha.sdk.tx.offline

import org.hyperledger.iroha.sdk.tx.SignedTransaction

/**
 * Norito-serialisable container that captures a signed transaction alongside
 * metadata required for offline submission or hand-off between devices.
 */
class OfflineSigningEnvelope(
    encodedPayload: ByteArray,
    signature: ByteArray,
    publicKey: ByteArray,
    @JvmField val schemaName: String,
    @JvmField val keyAlias: String,
    @JvmField val issuedAtMs: Long = System.currentTimeMillis(),
    metadata: Map<String, String> = emptyMap(),
    exportedKeyBundle: ByteArray? = null,
) {
    private val _encodedPayload: ByteArray = encodedPayload.copyOf()
    private val _signature: ByteArray = signature.copyOf()
    private val _publicKey: ByteArray = publicKey.copyOf()
    private val _metadata: Map<String, String> = metadata.toMap()
    private val _exportedKeyBundle: ByteArray? = exportedKeyBundle?.copyOf()

    init {
        require(schemaName.isNotBlank()) { "schemaName must not be blank" }
        require(keyAlias.isNotBlank()) { "keyAlias must not be blank" }
        require(issuedAtMs >= 0) { "issuedAtMs must be non-negative" }
        require(_encodedPayload.isNotEmpty()) { "encodedPayload must not be empty" }
        require(_signature.isNotEmpty()) { "signature must not be empty" }
        require(_publicKey.isNotEmpty()) { "publicKey must not be empty" }
    }

    /** Returns the canonical Norito payload bytes. */
    val encodedPayload: ByteArray get() = _encodedPayload.copyOf()

    /** Returns the raw signature bytes. */
    val signature: ByteArray get() = _signature.copyOf()

    /** Returns the public signing key bytes. */
    val publicKey: ByteArray get() = _publicKey.copyOf()

    /** Additional metadata supplied by the caller (immutable). */
    val metadata: Map<String, String> get() = _metadata

    /** Optional deterministic export of the signing key (if provided). */
    val exportedKeyBundle: ByteArray? get() = _exportedKeyBundle?.copyOf()

    /** Recreates a `SignedTransaction` instance from the envelope contents. */
    fun toSignedTransaction(): SignedTransaction = SignedTransaction(
        encodedPayload,
        signature,
        publicKey,
        schemaName,
        keyAlias,
        _exportedKeyBundle?.copyOf(),
    )

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is OfflineSigningEnvelope) return false
        return _encodedPayload.contentEquals(other._encodedPayload)
            && _signature.contentEquals(other._signature)
            && _publicKey.contentEquals(other._publicKey)
            && schemaName == other.schemaName
            && keyAlias == other.keyAlias
            && issuedAtMs == other.issuedAtMs
            && _metadata == other._metadata
            && _exportedKeyBundle.contentEqualsNullable(other._exportedKeyBundle)
    }

    override fun hashCode(): Int {
        var result = schemaName.hashCode()
        result = 31 * result + keyAlias.hashCode()
        result = 31 * result + issuedAtMs.hashCode()
        result = 31 * result + _metadata.hashCode()
        result = 31 * result + _encodedPayload.contentHashCode()
        result = 31 * result + _signature.contentHashCode()
        result = 31 * result + _publicKey.contentHashCode()
        result = 31 * result + (_exportedKeyBundle?.contentHashCode() ?: 0)
        return result
    }
}

private fun ByteArray?.contentEqualsNullable(other: ByteArray?): Boolean {
    if (this == null && other == null) return true
    if (this == null || other == null) return false
    return this.contentEquals(other)
}
