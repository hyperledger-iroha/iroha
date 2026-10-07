// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import java.io.Closeable
import java.util.concurrent.atomic.AtomicLong
import org.hyperledger.iroha.sdk.privacy.PrivacyNativeBridge

/** Native failure; -4 means the authenticated operation/Λ/Ω artifact loader is unavailable. */
class KagemushaWalletExceptionV1(
    @JvmField val status: Int,
    @JvmField val reason: Int = -1,
    @JvmField val platformCode: Int = 0,
) : IllegalStateException("KAGEMUSHA native status $status (reason $reason, platform $platformCode)") {
    companion object {
        /** There is deliberately no structural verifier or software-key fallback. */
        const val ARTIFACTS_UNAVAILABLE = -4
        /** Native output violates the fixed status/payload contract. */
        const val INVALID_NATIVE_OUTPUT = -100
        /** Missing, stale or unauthenticated native bridge library. */
        const val BRIDGE_UNAVAILABLE = -101
    }
}

/** Explicit custody outcome. Only COMPLETE permits the original result to be delivered. */
class KagemushaWalletCallV1 internal constructor(
    @JvmField val status: Int,
    @JvmField val reason: Int,
    @JvmField val platformCode: Int,
    @JvmField val sequenceLow: Long,
    @JvmField val sequenceHigh: Long,
    @JvmField val detail: Int,
    bytes: ByteArray,
) {
    init {
        val carriesBytes = status == COMPLETE || status == CREDIT_STATUS
        if ((status >= 0 && status !in UNKNOWN..PREPARING) || bytes.size > 10_000 ||
            (if (carriesBytes) bytes.isEmpty() else bytes.isNotEmpty())) {
            throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        }
    }
    private val retainedBytes = bytes.copyOf()
    /** Exact canonical bytes returned by Rust; this accessor never assembles or signs again. */
    fun bytes(): ByteArray = retainedBytes.copyOf()
    override fun toString(): String = "KagemushaWalletCallV1(status=$status, bytes=[REDACTED])"
    companion object {
        const val UNKNOWN = 0
        const val COMPLETE = 1
        const val PENDING = 2
        const val NOT_PERFORMED = 3
        const val ARCHIVED = 4
        const val DELIVERY_DATA_LOSS = 5
        const val IDLE = 6
        const val CAUGHT_UP = 7
        const val CHECKPOINT = 8
        const val FOLDED = 9
        const val CREDIT_STATUS = 10
        /** Durable intent exists, but irreversible Advance has not selected it. */
        const val PREPARING = 11
    }
}

/**
 * One exclusive native wallet owner. Calls may run on workers; activity and payment arrival
 * can preempt folding because this wrapper does not hold a managed lock across a native call.
 * Closing joins cooperative folding and retains every already committed payment in custody.
 *
 * [open] currently reports ArtifactsUnavailable until the authenticated native proof loader
 * is complete. The platform adapter and an ordinary filesystem are not monetary authority.
 */
class KagemushaWalletV1 private constructor(handle: Long) : Closeable {
    private val owner = AtomicLong(handle)
    private fun handle(): Long = owner.get().takeIf { it > 0 } ?: throw KagemushaWalletExceptionV1(-2)
    private fun call(operation: Int, first: ByteArray = byteArrayOf(), second: ByteArray = byteArrayOf()): KagemushaWalletCallV1 {
        val value = KagemushaWalletNativeV1.call(handle(), operation, first.copyOf(), second.copyOf())
            ?: throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        if (value.status < 0) throw KagemushaWalletExceptionV1(value.status, value.reason, value.platformCode)
        return value
    }
    private fun execute(input: KagemushaWalletOperationInputV1): KagemushaWalletCallV1 {
        val value = KagemushaWalletNativeV1.execute(handle(), input.requestId(), input.selector,
            input.amount.low, input.amount.high, input.first(), input.second(), input.third())
            ?: throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        if (value.status < 0) throw KagemushaWalletExceptionV1(value.status, value.reason, value.platformCode)
        return value
    }
    private fun setup(input: KagemushaWalletSetupInputV1): KagemushaWalletSetupReplyV1 {
        val value = KagemushaWalletNativeV1.setup(handle(), input.requestId(), input.selector,
            input.amount.low, input.amount.high, input.token, input.first(), input.second(), input.third())
            ?: throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        if (value.status < 0) throw KagemushaWalletExceptionV1(value.status, value.reason, value.platformCode)
        return value
    }
    /** Prove and release the admitted enrollment's initial head through the one native owner. */
    fun bootstrap(): KagemushaWalletCallV1 = setup(KagemushaWalletSetupInputV1(0)).completion()
    /** Sign and durably retain an Offer; retries use the same identity and positive amount. */
    fun offer(requestId: ByteArray, amount: KagemushaWalletUInt128V1): ByteArray =
        setup(KagemushaWalletSetupInputV1(1, requestId, amount)).original()
    /** Issue the exact receiver-signed Request; Rust selects the current head, maps and key. */
    fun issueRequest(requestId: ByteArray, offer: ByteArray, feeSchedule: ByteArray? = null,
        feeCertificate: ByteArray? = null): ByteArray {
        require((feeSchedule == null) == (feeCertificate == null)) { "fee schedule and certificate must be supplied together" }
        if (feeSchedule != null) require(feeSchedule.isNotEmpty() && feeCertificate!!.isNotEmpty())
        return setup(KagemushaWalletSetupInputV1(2, requestId, first = offer,
            second = feeSchedule ?: byteArrayOf(), third = feeCertificate ?: byteArrayOf())).original()
    }
    /** Verify delivery evidence and derive Archive from the permanent local Send index. */
    fun acceptCredited(credited: ByteArray): KagemushaWalletCallV1 =
        setup(KagemushaWalletSetupInputV1(3, first = credited)).completion()
    /** Start a real native direct time exchange. The caller supplies no clock or challenge. */
    fun beginDirectTimeExchange(): KagemushaWalletDirectTimeExchangeV1 =
        setup(KagemushaWalletSetupInputV1(4)).exchange(this)
    /** Consume this owner's native challenge and authenticate the actual signed reply. */
    fun finishDirectTimeExchange(exchange: KagemushaWalletDirectTimeExchangeV1,
        anchor: ByteArray, certificate: ByteArray) {
        // Validate/copy originals before consuming the one-use native challenge.
        val input = KagemushaWalletSetupInputV1(5, token = exchange.tokenFor(this), first = anchor, second = certificate)
        handle()
        exchange.consume(this)
        setup(input).timeRetained()
    }
    /** Load the exact ordinary-ledger receipt and compact finality evidence. */
    fun load(requestId: ByteArray, receipt: ByteArray, finality: ByteArray): KagemushaWalletCallV1 =
        execute(KagemushaWalletOperationInputV1(requestId, 0, first = receipt, second = finality))
    /** Irreversible Send; Rust authenticates the exact receiver-signed Request. */
    fun send(requestId: ByteArray, request: ByteArray): KagemushaWalletCallV1 =
        execute(KagemushaWalletOperationInputV1(requestId, 1, first = request))
    /** Receive selects the durably issued Request locally by the Payment's digest. */
    fun receive(requestId: ByteArray, payment: ByteArray, payerCredential: ByteArray, certificates: ByteArray): KagemushaWalletCallV1 =
        execute(KagemushaWalletOperationInputV1(requestId, 2, first = payment, second = payerCredential, third = certificates))
    /** Refresh from exact signed originals; Native derives maps and all effective values. */
    fun refresh(requestId: ByteArray, kind: KagemushaWalletRefreshKindV1, update: ByteArray, certificates: ByteArray): KagemushaWalletCallV1 =
        execute(KagemushaWalletOperationInputV1(requestId, kind.selector, first = update, second = certificates))
    /** Unload gross value to the credential account; an optional quote requires its certificates. */
    fun unload(requestId: ByteArray, amount: KagemushaWalletUInt128V1, quote: ByteArray? = null, certificates: ByteArray? = null): KagemushaWalletCallV1 {
        require((quote == null) == (certificates == null)) { "quote and certificates must be supplied together" }
        return execute(KagemushaWalletOperationInputV1(requestId, 8, amount, quote ?: byteArrayOf(), certificates ?: byteArrayOf()))
    }
    /** Enter Retiring under native folded-state checks. */
    fun retire(requestId: ByteArray): KagemushaWalletCallV1 = execute(KagemushaWalletOperationInputV1(requestId, 9))
    /** Resolve the local request identity, including distinct Preparing and irreversible Pending. */
    fun requestStatus(requestId: ByteArray): KagemushaWalletCallV1 {
        word(requestId); require(requestId.any { it != 0.toByte() }); return call(5, requestId)
    }
    /** Retrieve exact retained bytes; unknown, pending and delivery loss remain distinct. */
    fun retry(operationId: ByteArray): KagemushaWalletCallV1 { word(operationId); return call(1, operationId) }
    /** Reconcile and finish the selected operation without a second debit. */
    fun resume(): KagemushaWalletCallV1 = call(2)
    /** Compute at most one checkpoint. Run off the UI thread; activity enables background work. */
    fun foldOnce(): KagemushaWalletCallV1 = call(3)
    /** Native ownership and proof backlog, without operation readiness. Call on a worker. */
    fun snapshot(): KagemushaWalletSnapshotV1 {
        val reply = KagemushaWalletNativeV1.snapshot(handle()) ?: throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        if (reply.status < 0) throw KagemushaWalletExceptionV1(reply.status, reply.reason, reply.platformCode)
        return KagemushaWalletSnapshotV1(reply)
    }
    /** Canonical CreditStatus for the immutable first Payment identity. */
    fun creditStatus(creditId: ByteArray, paymentDigest: ByteArray): KagemushaWalletCallV1 {
        word(creditId); word(paymentDigest); return call(4, creditId, paymentDigest)
    }
    /** Foreground or charging enables folding; leaving both cancels at a cooperative boundary. */
    fun setActivity(foreground: Boolean, charging: Boolean) {
        checked(KagemushaWalletNativeV1.activity(handle(), if (foreground) 1 else 0, if (charging) 1 else 0))
    }
    /** Release exclusive native custody. Never deletes key material, markers or retained bytes. */
    override fun close() {
        val handle = owner.getAndSet(0)
        if (handle > 0) checked(KagemushaWalletNativeV1.close(handle))
    }
    override fun toString(): String = "KagemushaWalletV1(owner=[REDACTED])"
    companion object {
        private fun word(bytes: ByteArray) { require(bytes.size == 32) { "identity must be exactly32 bytes" } }
        private fun checked(status: Int) { if (status != 0) throw KagemushaWalletExceptionV1(status) }
        /**
         * Open through the authenticated SDK library and artifact owner. Every identity is
         * exactly32 nonzero bytes. No existing key is generated, replaced or deleted by this API.
         * TODO(G3/G4): native authenticated loader currently returns ArtifactsUnavailable.
         */
        @JvmStatic
        fun open(platform: KagemushaWalletAndroidPlatformV1, slot: ByteArray, scheme: ByteArray, wallet: ByteArray, artifact: ByteArray): KagemushaWalletV1 {
            for (id in listOf(slot, scheme, wallet, artifact)) { word(id); require(id.any { it != 0.toByte() }) { "identity must be nonzero" } }
            if (!PrivacyNativeBridge.isNativeAvailable()) throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.BRIDGE_UNAVAILABLE)
            val handle = try {
                if (KagemushaWalletNativeV1.revision() != 1) throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.BRIDGE_UNAVAILABLE)
                KagemushaWalletNativeV1.open(platform, slot.copyOf(), scheme.copyOf(), wallet.copyOf(), artifact.copyOf())
            } catch (_: LinkageError) { throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.BRIDGE_UNAVAILABLE) }
            if (handle <= 0) throw KagemushaWalletExceptionV1(if (handle < 0) handle.toInt() else KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
            return KagemushaWalletV1(handle)
        }
    }
}

/** JNI declarations share the current authenticated native library loader. */
internal object KagemushaWalletNativeV1 {
    @JvmStatic external fun revision(): Int
    @JvmStatic external fun open(platform: KagemushaWalletAndroidPlatformV1, slot: ByteArray, scheme: ByteArray, wallet: ByteArray, artifact: ByteArray): Long
    @JvmStatic external fun close(handle: Long): Int
    @JvmStatic external fun activity(handle: Long, foreground: Int, charging: Int): Int
    @JvmStatic external fun call(handle: Long, operation: Int, first: ByteArray, second: ByteArray): KagemushaWalletCallV1?
    @JvmStatic external fun execute(handle: Long, requestId: ByteArray, selector: Int, amountLow: Long, amountHigh: Long, first: ByteArray, second: ByteArray, third: ByteArray): KagemushaWalletCallV1?
    @JvmStatic external fun setup(handle: Long, requestId: ByteArray, selector: Int, amountLow: Long, amountHigh: Long, token: Long, first: ByteArray, second: ByteArray, third: ByteArray): KagemushaWalletSetupReplyV1?
    @JvmStatic external fun snapshot(handle: Long): KagemushaWalletSnapshotReplyV1?
}

/** JNI transport envelope only. Setup originals cannot become monetary completion. */
internal class KagemushaWalletSetupReplyV1(
    @JvmField val status: Int, @JvmField val reason: Int, @JvmField val platformCode: Int,
    @JvmField val sequenceLow: Long, @JvmField val sequenceHigh: Long, @JvmField val detail: Int,
    bytes: ByteArray,
) {
    private val retained = bytes.copyOf()
    init {
        if (bytes.size > 10_000 || (status < 0 && bytes.isNotEmpty())) invalid()
    }
    private fun invalid(): Nothing = throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
    fun completion(): KagemushaWalletCallV1 =
        KagemushaWalletCallV1(status, reason, platformCode, sequenceLow, sequenceHigh, detail, retained)
    fun original(): ByteArray {
        if (status != 12 || sequenceLow != 0L || sequenceHigh != 0L || detail != 0 || retained.isEmpty()) invalid()
        return retained.copyOf()
    }
    fun exchange(owner: Any): KagemushaWalletDirectTimeExchangeV1 {
        if (status != 13 || sequenceLow <= 0 || sequenceHigh != 0L || detail != 0 ||
            retained.size != 32 || retained.all { it == 0.toByte() }) invalid()
        return KagemushaWalletDirectTimeExchangeV1(owner, sequenceLow, retained)
    }
    fun timeRetained() {
        if (status != 14 || sequenceLow != 0L || sequenceHigh != 0L || detail != 0 || retained.isNotEmpty()) invalid()
    }
    override fun toString(): String = "KagemushaWalletSetupReplyV1(status=$status, bytes=[REDACTED])"
}

/** Move-only challenge bound to the same live native wallet owner. Not a caller-selected clock. */
class KagemushaWalletDirectTimeExchangeV1 internal constructor(
    private val origin: Any, private val token: Long, nonce: ByteArray,
) {
    private val remaining = AtomicLong(token)
    private val retainedNonce = nonce.copyOf()
    /** Send these exact original challenge bytes to the configured signed time service. */
    fun nonce(): ByteArray = retainedNonce.copyOf()
    internal fun tokenFor(owner: Any): Long {
        require(owner === origin && remaining.get() == token) { "time exchange is closed or belongs to another wallet" }
        return token
    }
    internal fun consume(owner: Any) {
        require(owner === origin && remaining.compareAndSet(token, 0)) { "time exchange is closed or belongs to another wallet" }
    }
    override fun toString(): String = "KagemushaWalletDirectTimeExchangeV1(challenge=[REDACTED])"
}

/** Fixed setup input. Rust still performs all original decoding, signature and source checks. */
internal class KagemushaWalletSetupInputV1(
    val selector: Int, requestId: ByteArray = ByteArray(32),
    val amount: KagemushaWalletUInt128V1 = KagemushaWalletUInt128V1(0, 0), val token: Long = 0,
    first: ByteArray = byteArrayOf(), second: ByteArray = byteArrayOf(), third: ByteArray = byteArrayOf(),
) {
    private val identity: ByteArray
    private val originals: List<ByteArray>
    init {
        val limits = when (selector) {
            0, 1, 4 -> intArrayOf(0, 0, 0)
            2 -> intArrayOf(10_000, 1_024, 512)
            3 -> intArrayOf(10_000, 0, 0)
            5 -> intArrayOf(512, 512, 0)
            else -> throw IllegalArgumentException("unknown setup operation")
        }
        require(requestId.size == 32 && ((selector == 1 || selector == 2) == requestId.any { it != 0.toByte() }))
        require((selector == 1) == (amount.low != 0L || amount.high != 0L))
        require(if (selector == 5) token > 0 else token == 0L)
        val inputs = listOf(first, second, third)
        inputs.forEachIndexed { index, bytes -> require(bytes.size <= limits[index]) }
        if (selector == 2) require(first.isNotEmpty() && second.isEmpty() == third.isEmpty())
        if (selector == 3) require(first.isNotEmpty())
        if (selector == 5) require(first.isNotEmpty() && second.isNotEmpty())
        identity = requestId.copyOf()
        originals = inputs.map { it.copyOf() }
    }
    fun requestId(): ByteArray = identity.copyOf()
    fun first(): ByteArray = originals[0].copyOf()
    fun second(): ByteArray = originals[1].copyOf()
    fun third(): ByteArray = originals[2].copyOf()
    override fun toString(): String = "KagemushaWalletSetupInputV1(originals=[REDACTED])"
}

/** Existing signed policy classes; these selectors never choose proof keys or state roots. */
enum class KagemushaWalletRefreshKindV1(internal val selector: Int) {
    CREDENTIAL(3), SCHEME_POLICY(4), BLACKLIST(5), TIME_ANCHOR(6), QUOTA_SHARE(7),
}

/** Allocation-bounded copies for the fixed JNI request. Validation is not monetary admission. */
internal class KagemushaWalletOperationInputV1(
    requestId: ByteArray,
    val selector: Int,
    val amount: KagemushaWalletUInt128V1 = KagemushaWalletUInt128V1(0, 0),
    first: ByteArray = byteArrayOf(), second: ByteArray = byteArrayOf(), third: ByteArray = byteArrayOf(),
) {
    private val identity: ByteArray
    private val originals: List<ByteArray>
    init {
        val limits = when (selector) {
            0 -> intArrayOf(512, 16_384, 0)
            1 -> intArrayOf(10_000, 0, 0)
            2 -> intArrayOf(10_000, 1_024, 10_000)
            3, 4 -> intArrayOf(1_024, 10_000, 0)
            5 -> intArrayOf(65_536 * 34 + 512, 10_000, 0)
            6 -> intArrayOf(512, 10_000, 0)
            7 -> intArrayOf(8_192, 10_000, 0)
            8 -> intArrayOf(1_024, 10_000, 0)
            9 -> intArrayOf(0, 0, 0)
            else -> throw IllegalArgumentException("unknown lifecycle operation")
        }
        require(requestId.size == 32 && requestId.any { it != 0.toByte() }) { "nonzero request identity must be exactly 32 bytes" }
        val nonzero = amount.low != 0L || amount.high != 0L
        require(if (selector == 8) nonzero else !nonzero) { "only Unload has a positive amount" }
        val inputs = listOf(first, second, third)
        for (index in inputs.indices) {
            require(inputs[index].size <= limits[index]) { "original exceeds operation bound" }
            if (selector < 8 && limits[index] != 0) require(inputs[index].isNotEmpty()) { "required original is empty" }
        }
        if (selector == 8) require(first.isEmpty() == second.isEmpty()) { "quote and certificates must be supplied together" }
        identity = requestId.copyOf()
        originals = inputs.map { it.copyOf() }
    }
    fun requestId(): ByteArray = identity.copyOf()
    fun first(): ByteArray = originals[0].copyOf()
    fun second(): ByteArray = originals[1].copyOf()
    fun third(): ByteArray = originals[2].copyOf()
    override fun toString(): String = "KagemushaWalletOperationInputV1(originals=[REDACTED])"
}
