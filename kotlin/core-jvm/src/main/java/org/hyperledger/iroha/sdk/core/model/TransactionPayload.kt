package org.hyperledger.iroha.sdk.core.model

import org.hyperledger.iroha.sdk.address.AccountAddress

private const val DEFAULT_CHAIN_ID = "00000000"
private val DEFAULT_AUTHORITY = AccountAddress
    .fromAccount(ByteArray(32), "ed25519")
    .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)

/**
 * Representation of a transaction payload prior to Norito encoding.
 *
 * The structure mirrors the Rust data model sufficiently for encoding and signing. Instruction
 * handling currently focuses on the IVM bytecode variant; support for general instruction lists will
 * be added alongside dedicated builders. `authority` must use the canonical public I105 account
 * literal.
 */
class TransactionPayload(
    val chainId: String = DEFAULT_CHAIN_ID,
    val authority: String = DEFAULT_AUTHORITY,
    val creationTimeMs: Long = System.currentTimeMillis(),
    val executable: Executable = Executable.ivm(byteArrayOf()),
    val timeToLiveMs: Long? = null,
    val nonce: Int? = null,
    metadata: Map<String, String> = emptyMap(),
) {
    private val _metadata: Map<String, String> = metadata.toMap()

    val metadata: Map<String, String> get() = _metadata

    init {
        require(chainId.isNotBlank()) { "chainId must not be blank" }
        require(authority.isNotBlank()) { "authority must not be blank" }
        require(creationTimeMs >= 0) { "creationTimeMs must be non-negative" }
        if (timeToLiveMs != null) {
            require(timeToLiveMs > 0) { "timeToLiveMs must be positive when present" }
        }
        if (nonce != null) {
            require(nonce > 0) { "nonce must be positive when present" }
        }
        _metadata.keys.forEach { key ->
            require(key.isNotBlank()) { "metadata key must not be blank" }
        }
    }

    fun copy(
        chainId: String = this.chainId,
        authority: String = this.authority,
        creationTimeMs: Long = this.creationTimeMs,
        executable: Executable = this.executable,
        timeToLiveMs: Long? = this.timeToLiveMs,
        nonce: Int? = this.nonce,
        metadata: Map<String, String> = this.metadata,
    ): TransactionPayload = TransactionPayload(
        chainId = chainId,
        authority = authority,
        creationTimeMs = creationTimeMs,
        executable = executable,
        timeToLiveMs = timeToLiveMs,
        nonce = nonce,
        metadata = metadata,
    )

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is TransactionPayload) return false
        return chainId == other.chainId
            && authority == other.authority
            && creationTimeMs == other.creationTimeMs
            && executable == other.executable
            && timeToLiveMs == other.timeToLiveMs
            && nonce == other.nonce
            && _metadata == other._metadata
    }

    override fun hashCode(): Int {
        var result = chainId.hashCode()
        result = 31 * result + authority.hashCode()
        result = 31 * result + creationTimeMs.hashCode()
        result = 31 * result + executable.hashCode()
        result = 31 * result + (timeToLiveMs?.hashCode() ?: 0)
        result = 31 * result + (nonce?.hashCode() ?: 0)
        result = 31 * result + _metadata.hashCode()
        return result
    }
}
