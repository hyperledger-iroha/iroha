package org.hyperledger.iroha.sdk.offline

/** Immutable view over a pending offline journal entry. */
class OfflineJournalEntry internal constructor(
    txId: ByteArray,
    payload: ByteArray,
    val recordedAtMs: Long,
    hashChain: ByteArray,
) {
    private val _txId: ByteArray = txId.copyOf()
    private val _payload: ByteArray = payload.copyOf()
    private val _hashChain: ByteArray = hashChain.copyOf()

    val txId: ByteArray get() = _txId.copyOf()
    val payload: ByteArray get() = _payload.copyOf()
    val hashChain: ByteArray get() = _hashChain.copyOf()
}
