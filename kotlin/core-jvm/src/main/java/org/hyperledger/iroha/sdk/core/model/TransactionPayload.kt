package org.hyperledger.iroha.sdk.core.model

import java.util.Collections
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address
import org.hyperledger.iroha.sdk.core.model.instructions.ProofAttachment

private const val MAX_U32 = 0xffff_ffffL
private const val DEFAULT_TRANSACTION_TTL_MS = 100_000L
private const val MAX_CHAIN_ID_BYTES = 128

/**
 * Representation of a transaction payload prior to Norito encoding.
 *
 * The structure mirrors the Rust data model sufficiently for encoding and signing, including
 * instruction lists, by-reference contract calls, IVM bytecode, and flat mixed batches. `authority`
 * must use the canonical I105 account literal. `nonce` uses a [Long] carrier for the full nonzero
 * unsigned 32-bit wire range. Proof attachments are part of the signed payload and therefore
 * affect both authorization signatures and the canonical transaction identifier.
 */
class TransactionPayload(
    val chainId: String,
    val authority: String,
    val creationTimeMs: Long = System.currentTimeMillis(),
    val executable: Executable = Executable.ivm(byteArrayOf()),
    val timeToLiveMs: Long? = DEFAULT_TRANSACTION_TTL_MS,
    val nonce: Long? = null,
    val feePayment: FeePaymentIntent,
    metadata: Map<String, JsonValue> = emptyMap(),
    attachments: List<ProofAttachment>? = null,
) {
    private val _metadata: Map<String, JsonValue> = metadata.toMap()
    private val _attachments: List<ProofAttachment>? =
        attachments?.let { values ->
            Collections.unmodifiableList(
                values.map { requireNotNull(it) { "attachments must not contain null" } },
            )
        }

    val metadata: Map<String, JsonValue> get() = _metadata

    /** Ordered execution proof attachments included in the signed transaction intent. */
    val attachments: List<ProofAttachment>? get() = _attachments

    init {
        requireCanonicalChainId(chainId)
        requireCanonicalI105Address(authority, "authority")
        require(creationTimeMs >= 0) { "creationTimeMs must be non-negative" }
        require(timeToLiveMs != null && timeToLiveMs > 0) {
            "timeToLiveMs must be a positive signature-bound lifetime"
        }
        if (nonce != null) {
            require(nonce in 1..MAX_U32) { "nonce must fit in the nonzero u32 range" }
        }
        _metadata.keys.forEach { key ->
            require(key.isNotBlank()) { "metadata key must not be blank" }
        }
    }

    private fun requireCanonicalChainId(value: String) {
        require(value.isNotEmpty() && value.length <= MAX_CHAIN_ID_BYTES) {
            "chainId must contain 1..$MAX_CHAIN_ID_BYTES ASCII bytes"
        }
        require(value.first().isAsciiLetterOrDigit() && value.last().isAsciiLetterOrDigit()) {
            "chainId must begin and end with an ASCII alphanumeric character"
        }
        require(value.all { character ->
            character.isAsciiLetterOrDigit() || character == '.' || character == '_' ||
                character == ':' || character == '-'
        }) {
            "chainId contains a non-canonical character"
        }
    }

    private fun Char.isAsciiLetterOrDigit(): Boolean =
        this in 'a'..'z' || this in 'A'..'Z' || this in '0'..'9'

    fun copy(
        chainId: String = this.chainId,
        authority: String = this.authority,
        creationTimeMs: Long = this.creationTimeMs,
        executable: Executable = this.executable,
        timeToLiveMs: Long? = this.timeToLiveMs,
        nonce: Long? = this.nonce,
        feePayment: FeePaymentIntent = this.feePayment,
        metadata: Map<String, JsonValue> = this.metadata,
        attachments: List<ProofAttachment>? = this.attachments,
    ): TransactionPayload = TransactionPayload(
        chainId = chainId,
        authority = authority,
        creationTimeMs = creationTimeMs,
        executable = executable,
        timeToLiveMs = timeToLiveMs,
        nonce = nonce,
        feePayment = feePayment,
        metadata = metadata,
        attachments = attachments,
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
            && feePayment == other.feePayment
            && _metadata == other._metadata
            && _attachments == other._attachments
    }

    override fun hashCode(): Int {
        var result = chainId.hashCode()
        result = 31 * result + authority.hashCode()
        result = 31 * result + creationTimeMs.hashCode()
        result = 31 * result + executable.hashCode()
        result = 31 * result + (timeToLiveMs?.hashCode() ?: 0)
        result = 31 * result + (nonce?.hashCode() ?: 0)
        result = 31 * result + feePayment.hashCode()
        result = 31 * result + _metadata.hashCode()
        result = 31 * result + (_attachments?.hashCode() ?: 0)
        return result
    }
}
