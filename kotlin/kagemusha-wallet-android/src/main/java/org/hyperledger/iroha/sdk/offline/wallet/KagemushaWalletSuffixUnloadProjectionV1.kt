// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import java.math.BigInteger
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash
import org.hyperledger.iroha.sdk.offline.KagemushaP256Codec

/** Native-selected financial display DATA; parsing cannot create a review capability. */
class KagemushaWalletSuffixUnloadReviewProjectionV1 internal constructor(original: ByteArray) {
    private val retained = original.takeIf { it.size in 48..12_288 }?.copyOf() ?: invalidSuffixOutput()
    private val request: ByteArray
    private val authority: ByteArray
    private val destination: ByteArray
    private val destinationOriginal: ByteArray
    val amount: KagemushaWalletUInt128V1
    val charge: KagemushaWalletUInt128V1
    val fee: KagemushaWalletUInt128V1 get() = charge
    val grossDebit: KagemushaWalletUInt128V1
    val netDestinationAmount: KagemushaWalletUInt128V1
    val stepCount: KagemushaWalletUInt128V1
    val chunkCount: KagemushaWalletUInt128V1
    val instructionCount: KagemushaWalletUInt128V1

    init {
        val r = suffixFrame(retained, "SuffixUnloadReviewProjectionV1", 13, 12_288)
        request = r.identity()
        r.identity() // scheme
        r.identity() // wallet
        authority = r.identity()
        r.identity() // source operation
        r.identity() // source capsule
        r.identity() // source state
        val sequence = r.scalar()
        val anchor = r.scalar()
        stepCount = r.scalar(); chunkCount = r.scalar(); instructionCount = r.scalar()
        suffixCounts(stepCount, chunkCount, instructionCount)
        requireSuffix(anchor.big() + stepCount.big() == sequence.big())
        amount = r.scalar(); charge = r.scalar(); grossDebit = r.scalar(); netDestinationAmount = r.scalar()
        requireSuffix(amount.big().signum() > 0 && grossDebit == amount && charge <= amount &&
            amount.big() - charge.big() == netDestinationAmount.big())
        destination = r.identity()
        destinationOriginal = r.bytes(4_096)
        requireSuffix(destinationOriginal.isNotEmpty())
        val quote = r.word()
        val beneficiary = r.optional { it.rawVector(4_096) }
        val hasQuote = quote.any { it != 0.toByte() }
        requireSuffix(hasQuote == (beneficiary != null) && (hasQuote || charge.big().signum() == 0))
        requireSuffix(beneficiary == null || beneficiary.isNotEmpty())
        r.identity() // credential
        r.identity() // installed artifact manifest
        val key = r.field { it.raw(65) }
        try { KagemushaP256Codec.requireUncompressedPublicKey(key) }
        catch (_: IllegalArgumentException) { invalidSuffixOutput() }
        r.finish()
    }

    fun requestId(): ByteArray = request.copyOf()
    fun authorityDigest(): ByteArray = authority.copyOf()
    fun destinationAccountDigest(): ByteArray = destination.copyOf()
    fun destinationAccountOriginal(): ByteArray = destinationOriginal.copyOf()
    fun bytes(): ByteArray = retained.copyOf()
    override fun toString(): String = "KagemushaWalletSuffixUnloadReviewProjectionV1(source=[REDACTED])"
}

/** A completed native verification turn, not financial approval. */
class KagemushaWalletSuffixUnloadProgressV1 internal constructor(original: ByteArray) {
    private val request: ByteArray
    val completedSteps: KagemushaWalletUInt128V1
    val totalSteps: KagemushaWalletUInt128V1
    init {
        val r = suffixFrame(original, "SuffixUnloadProgressV1", 14, 256)
        request = r.identity(); completedSteps = r.scalar(); totalSteps = r.scalar(); r.finish()
        suffixProgress(completedSteps, totalSteps)
    }
    fun requestId(): ByteArray = request.copyOf()
    override fun toString(): String = "KagemushaWalletSuffixUnloadProgressV1(source=[REDACTED])"
}

/** Durable offline debit and immutable stage inventory. Ledger payout is a separate outcome. */
class KagemushaWalletSuffixUnloadReleasedV1 internal constructor(original: ByteArray) {
    private val request: ByteArray
    private val authority: ByteArray
    val amount: KagemushaWalletUInt128V1
    val stepCount: KagemushaWalletUInt128V1
    val chunkCount: KagemushaWalletUInt128V1
    val instructionCount: KagemushaWalletUInt128V1
    init {
        val r = suffixFrame(original, "SuffixUnloadReleasedV1", 16, 512)
        request = r.identity(); authority = r.identity(); amount = r.scalar()
        stepCount = r.scalar(); chunkCount = r.scalar(); instructionCount = r.scalar(); r.finish()
        requireSuffix(amount.big().signum() > 0)
        suffixCounts(stepCount, chunkCount, instructionCount)
    }
    fun requestId(): ByteArray = request.copyOf()
    fun authorityDigest(): ByteArray = authority.copyOf()
    override fun toString(): String = "KagemushaWalletSuffixUnloadReleasedV1(source=[REDACTED])"
}

/** O(1) wallet scheduling observation. Every operation still checks its own native prerequisites. */
class KagemushaWalletSuffixUnloadStatusV1 internal constructor(original: ByteArray) {
    enum class State { INITIALIZING, NOT_APPLICABLE, ELIGIBLE, VERIFYING, READY, PENDING, RELEASED }
    val state: State
    val reason: Int
    val currentSequence: KagemushaWalletUInt128V1?
    val foldedSequence: KagemushaWalletUInt128V1?
    private val request: ByteArray?
    val completedSteps: KagemushaWalletUInt128V1
    val totalSteps: KagemushaWalletUInt128V1
    init {
        val r = suffixFrame(original, "SuffixUnloadStatusV1", 15, 512)
        val code = r.field { it.u16() }
        requireSuffix(code in State.values().indices)
        state = State.values()[code]
        reason = r.field { it.u16() }
        currentSequence = r.optional { it.u128() }
        foldedSequence = r.optional { it.u128() }
        request = r.optional { it.arrayElements(32) }
        completedSteps = r.scalar(); totalSteps = r.scalar(); r.finish()
        requireSuffix(if (state == State.NOT_APPLICABLE) reason in 1..3 else reason == 0)
        requireSuffix(foldedSequence == null || currentSequence != null && foldedSequence <= currentSequence)
        requireSuffix(request == null || request.any { it != 0.toByte() })
        if (code >= State.VERIFYING.ordinal) {
            requireSuffix(request != null)
            suffixProgress(completedSteps, totalSteps)
            if (code >= State.READY.ordinal) requireSuffix(completedSteps == totalSteps)
        } else {
            requireSuffix(request == null && completedSteps.big().signum() == 0 && totalSteps.big().signum() == 0)
        }
    }
    fun requestId(): ByteArray? = request?.copyOf()
    override fun toString(): String = "KagemushaWalletSuffixUnloadStatusV1(state=$state)"
}

/** Exact positive absence for a named resume only. It grants no preparation or debit authority. */
internal fun requireSuffixRequestAbsent(original: ByteArray, requestId: ByteArray) {
    requireSuffix(requestId.size == 32 && requestId.any { it != 0.toByte() })
    val r = suffixFrame(original, "SuffixUnloadStatusV1", 15, 512)
    requireSuffix(r.field { it.u16() } == 7 && r.field { it.u16() } == 0)
    val current = r.optional { it.u128() }
    val folded = r.optional { it.u128() }
    val request = r.optional { it.arrayElements(32) }
    val completed = r.scalar(); val total = r.scalar(); r.finish()
    requireSuffix(current != null && (folded == null || folded <= current))
    requireSuffix(request != null && request.contentEquals(requestId) &&
        completed.big().signum() == 0 && total.big().signum() == 0)
}

internal fun invalidSuffixOutput(): Nothing =
    throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)

internal fun requireSuffix(condition: Boolean) { if (!condition) invalidSuffixOutput() }

internal fun KagemushaWalletUInt128V1.big(): BigInteger =
    BigInteger(java.lang.Long.toUnsignedString(high)).shiftLeft(64) + BigInteger(java.lang.Long.toUnsignedString(low))

private fun suffixProgress(completed: KagemushaWalletUInt128V1, total: KagemushaWalletUInt128V1) {
    requireSuffix(total.big() in BigInteger.ONE..BigInteger.valueOf(4_096) && completed <= total)
}

private fun suffixCounts(steps: KagemushaWalletUInt128V1, chunks: KagemushaWalletUInt128V1,
    instructions: KagemushaWalletUInt128V1) {
    val count = chunks.big()
    requireSuffix(count in BigInteger.ONE..BigInteger.valueOf(512) &&
        steps.big() in count..count.multiply(BigInteger.valueOf(8)) && instructions.big() == count + BigInteger.valueOf(2))
}

private fun suffixFrame(original: ByteArray, name: String, discriminator: Int, maximum: Int): SuffixFields {
    val fields = suffixCanonicalFields(original,
        "iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::$name", maximum)
    requireSuffix(fields.field { it.u16() } == 1 && fields.field { it.u16() } == discriminator)
    return fields
}

/** Canonical outer carriage only. Native and ledger still authenticate every nested original. */
internal fun suffixInstructionOriginal(original: ByteArray): ByteArray {
    val r = suffixCanonicalFields(original, "iroha_data_model::isi::kagemusha_wallet::KagemushaWalletLedgerV1",
        KagemushaWalletSuffixUnloadV1.INSTRUCTION_MAX_BYTES)
    r.identity()
    r.field { action ->
        val tag = action.u32()
        requireSuffix(tag in 11L..13L)
        action.identity(); action.identity()
        action.field { requireSuffix(it.remainder(4_096).isNotEmpty()) }
        val ordinal = action.scalar()
        requireSuffix(ordinal.high == 0L && when (tag) {
            11L -> ordinal.low == 0L
            12L -> ordinal.low in 1L..512L
            else -> ordinal.low in 2L..513L
        })
        requireSuffix(action.bytes(KagemushaWalletSuffixUnloadV1.INSTRUCTION_MAX_BYTES).isNotEmpty())
        if (tag == 11L) requireSuffix(action.bytes(KagemushaWalletSuffixUnloadV1.INSTRUCTION_MAX_BYTES).isNotEmpty())
    }
    r.finish()
    return original.copyOf()
}

private fun suffixCanonicalFields(original: ByteArray, schema: String, maximum: Int): SuffixFields {
    try {
        require(original.size in 48..maximum)
        val decoded = NoritoHeader.decode(original, SchemaHash.hash16(schema))
        require(decoded.header.compression == NoritoHeader.COMPRESSION_NONE &&
            decoded.header.flags == NoritoHeader.COMPACT_LEN && original.size == 48 + decoded.payload.size)
        decoded.header.validateChecksum(decoded.payload)
        return SuffixFields(decoded.payload)
    } catch (_: IllegalArgumentException) { invalidSuffixOutput() }
}

/** Bounds-first reader of the declared COMPACT_LEN fields, not a second monetary codec. */
private class SuffixFields(bytes: ByteArray) {
    private val decoder = NoritoDecoder(bytes, NoritoHeader.COMPACT_LEN)
    fun <T> field(read: (SuffixFields) -> T): T = checked {
        val size = decoder.readLength(true)
        require(size <= decoder.remaining())
        val child = SuffixFields(decoder.readBytes(size.toInt()))
        val value = read(child); child.finish(); value
    }
    fun <T> optional(read: (SuffixFields) -> T): T? = field { child ->
        when (child.byte()) { 0 -> null; 1 -> child.field(read); else -> invalidSuffixOutput() }
    }
    fun word(): ByteArray = field { it.fixedArray(32) }
    fun identity(): ByteArray = word().also { requireSuffix(it.any { b -> b != 0.toByte() }) }
    fun scalar(): KagemushaWalletUInt128V1 = field { it.u128() }
    fun bytes(maximum: Int): ByteArray = field { it.rawVector(maximum) }
    fun fixedArray(size: Int): ByteArray = raw(size)
    fun arrayElements(size: Int): ByteArray = ByteArray(size) { field { it.byte().toByte() } }
    fun rawVector(maximum: Int): ByteArray = checked {
        val size = decoder.readLength(false)
        require(size <= maximum && size <= decoder.remaining())
        decoder.readBytes(size.toInt())
    }
    fun u128(): KagemushaWalletUInt128V1 = checked { KagemushaWalletUInt128V1(decoder.readUInt(64), decoder.readUInt(64)) }
    fun u16(): Int = checked { decoder.readUInt(16).toInt() }
    fun u32(): Long = checked { decoder.readUInt(32) }
    fun byte(): Int = checked { decoder.readByte() }
    fun raw(size: Int): ByteArray = checked { decoder.readBytes(size) }
    fun remainder(maximum: Int): ByteArray = checked {
        require(decoder.remaining() <= maximum)
        decoder.readBytes(decoder.remaining())
    }
    fun finish() { requireSuffix(decoder.remaining() == 0) }
    private fun <T> checked(read: () -> T): T = try { read() }
        catch (_: IllegalArgumentException) { invalidSuffixOutput() }
}
