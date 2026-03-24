package org.hyperledger.iroha.sdk.multisig

import java.util.TreeMap

private const val MAX_SIGNATORIES = 0xFF
private const val MAX_WEIGHT = 0xFF
private const val MAX_QUORUM = 0xFFFF

/** Multisig specification: signatories with weights, quorum, and transaction TTL. */
class MultisigSpec(
    signatories: Map<String, Int>,
    @JvmField val quorum: Int,
    @JvmField val transactionTtlMs: Long,
) {

    private val _signatories: Map<String, Int> = signatories.toMap()

    val signatories: Map<String, Int> get() = _signatories

    init {
        require(quorum > 0) { "quorum must be greater than zero" }
        require(quorum <= MAX_QUORUM) { "quorum must fit in an unsigned 16-bit integer" }
        require(transactionTtlMs > 0) { "transactionTtlMs must be greater than zero" }
        require(_signatories.isNotEmpty()) { "multisig specs require at least one signatory" }
        require(_signatories.size <= MAX_SIGNATORIES) { "multisig specs support at most 255 signatories" }
        for ((accountId, weight) in _signatories) {
            require(accountId.isNotBlank()) { "accountId must be a non-empty string" }
            require(weight > 0) { "weight must be greater than zero" }
            require(weight <= MAX_WEIGHT) { "weight must fit in an unsigned byte (max 255)" }
        }
        val totalWeight = _signatories.values.sumOf { it.toLong() }
        require(totalWeight >= quorum) { "quorum $quorum exceeds total signatory weight $totalWeight" }
    }

    fun previewProposalExpiry(requestedTtlMs: Long?, nowMs: Long): MultisigProposalTtlPreview {
        val normalizedNow = maxOf(0L, nowMs)
        val cap = transactionTtlMs
        val requested = if (requestedTtlMs == null) cap else {
            require(requestedTtlMs > 0) { "requestedTtlMs must be greater than zero" }
            requestedTtlMs
        }
        val wasCapped = requested > cap
        val effective = if (wasCapped) cap else requested
        val expiresAt = safeExpiry(normalizedNow, effective)
        return MultisigProposalTtlPreview(effective, cap, expiresAt, wasCapped)
    }

    fun enforceProposalTtl(requestedTtlMs: Long?, nowMs: Long): MultisigProposalTtlPreview {
        if (requestedTtlMs != null && requestedTtlMs > transactionTtlMs) {
            throw IllegalArgumentException(
                "Requested multisig TTL $requestedTtlMs ms exceeds the policy cap " +
                    "$transactionTtlMs ms; choose a value at or below the cap."
            )
        }
        return previewProposalExpiry(requestedTtlMs, nowMs)
    }

    @JvmOverloads
    fun toJson(prettyPrinted: Boolean = false): String {
        val newline = if (prettyPrinted) "\n" else ""
        val indent = if (prettyPrinted) "  " else ""
        val innerIndent = if (prettyPrinted) "    " else ""

        return buildString {
            append("{").append(newline)
            append(indent).append("\"signatories\": {")

            val sorted = TreeMap(_signatories)
            var first = true
            for ((key, value) in sorted) {
                if (!first) append(",")
                append(newline)
                    .append(innerIndent)
                    .append("\"")
                    .append(key)
                    .append("\": ")
                    .append(value)
                first = false
            }
            append(newline).append(indent).append("},").append(newline)
            append(indent).append("\"quorum\": ").append(quorum).append(",").append(newline)
            append(indent).append("\"transaction_ttl_ms\": ").append(transactionTtlMs).append(newline)
            append("}")
        }
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is MultisigSpec) return false
        return quorum == other.quorum
            && transactionTtlMs == other.transactionTtlMs
            && _signatories == other._signatories
    }

    override fun hashCode(): Int = listOf(_signatories, quorum, transactionTtlMs).hashCode()
}

private fun safeExpiry(nowMs: Long, ttlMs: Long): Long =
    try {
        Math.addExact(nowMs, ttlMs)
    } catch (_: ArithmeticException) {
        Long.MAX_VALUE
    }
