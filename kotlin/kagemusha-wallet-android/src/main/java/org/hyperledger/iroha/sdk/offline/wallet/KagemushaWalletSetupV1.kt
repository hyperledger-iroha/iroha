package org.hyperledger.iroha.sdk.offline.wallet

import java.util.concurrent.atomic.AtomicLong

/** Opaque single-use native exchange. No caller clock or decoder is available. */
class KagemushaWalletTimeExchangeV1 internal constructor(private val origin: Any, private val token: Long, nonce: ByteArray) {
    private val remaining = AtomicLong(token)
    private val original = nonce.copyOf()
    init {
        require(token > 0 && original.size == 32 && original.any { it != 0.toByte() }) { "native time challenge" }
    }
    /** Fresh native nonce to transmit to the signed time service. */
    fun nonce(): ByteArray = original.copyOf()
    internal fun tokenFor(owner: Any): Long {
        require(owner === origin && remaining.get() == token) { "time exchange is closed or belongs to another wallet" }
        return token
    }
    internal fun consume(owner: Any) {
        require(owner === origin && remaining.compareAndSet(token, 0)) { "time exchange is closed or belongs to another wallet" }
    }
    override fun toString(): String = "KagemushaWalletTimeExchangeV1(challenge=[REDACTED])"
}

/** Fixed intake and defensive copies only; Native authenticates and derives every field. */
internal class KagemushaWalletSetupInputV1(
    val selector: Int,
    identity: ByteArray = ByteArray(32),
    val amount: KagemushaWalletUInt128V1 = KagemushaWalletUInt128V1(0, 0),
    val token: Long = 0,
    first: ByteArray = byteArrayOf(), second: ByteArray = byteArrayOf(), third: ByteArray = byteArrayOf(),
) {
    private val id: ByteArray
    private val a: ByteArray
    private val b: ByteArray
    private val c: ByteArray
    init {
        val limits = when (selector) {
            0, 1, 4, 6, 15, 18, 19, 20, 24 -> intArrayOf(0, 0, 0)
            21, 22 -> intArrayOf(21_024, 0, 0)
            23 -> intArrayOf(36 * 1024 * 1024, 0, 0)
            25 -> intArrayOf(32 * 1024 * 1024, 1024, 0)
            26 -> intArrayOf(21_024, 16_384, 0)
            27 -> intArrayOf(16_384, 0, 0)
            2 -> intArrayOf(10_000, 1_024, 512)
            3, in 7..14, in 16..17 -> intArrayOf(10_000, 0, 0)
            5 -> intArrayOf(512, 512, 0)
            else -> throw IllegalArgumentException("unknown setup action")
        }
        require(identity.size == 32) { "setup identity" }
        require(first.size <= limits[0] && second.size <= limits[1] && third.size <= limits[2]) { "setup input bound" }
        // Array lengths cannot change; validate values on the bounded copies we retain.
        id = identity.copyOf()
        a = first.copyOf()
        b = second.copyOf()
        c = third.copyOf()
        require((selector in listOf(1, 2, 19, 20, 25, 27)) == id.any { it != 0.toByte() }) { "setup identity" }
        require((selector == 1) == (amount.low != 0L || amount.high != 0L)) { "Offer amount" }
        require(token >= 0 && ((selector == 5 || selector == 6) == (token != 0L))) { "native time token" }
        require(selector != 2 || (a.isNotEmpty() && b.isEmpty() == c.isEmpty())) { "Request originals" }
        require((selector != 3 && selector !in 7..14 && selector !in 16..17 && selector !in 21..23) || a.isNotEmpty()) { "Credited original" }
        require(selector !in listOf(5, 25, 26) || (a.isNotEmpty() && b.isNotEmpty())) { "time response originals" }
    }
    fun identity(): ByteArray = id.copyOf()
    fun first(): ByteArray = a.copyOf()
    fun second(): ByteArray = b.copyOf()
    fun third(): ByteArray = c.copyOf()
    override fun toString(): String = "KagemushaWalletSetupInputV1(originals=[REDACTED])"
}

/** Exact peer-envelope kind; this cannot select proof keys or operation authority. */
enum class KagemushaWalletTransportKindV1(internal val tag: Int) {
    OFFER(1), REQUEST(2), PAYMENT(3), CREDITED(4),
}

/** Worker scheduling state; use snapshot for the durable current backlog. */
class KagemushaWalletBackgroundStatusV1 internal constructor(value: KagemushaWalletCallV1) {
    enum class Phase { NOT_STARTED, PARKED, RUNNING }
    val phase: Phase
    val eligible: Boolean
    /** Last durable backlog observed by the worker, or null before its first observation. */
    val observedBacklog: KagemushaWalletUInt128V1?
    init {
        if (value.status != 29 || value.bytes().isNotEmpty() || value.detail and 15.inv() != 0 ||
            value.detail and 3 == 3 || (value.detail and 8 == 0 && (value.sequenceLow != 0L || value.sequenceHigh != 0L))) {
            throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        }
        phase = Phase.values()[value.detail and 3]
        eligible = value.detail and 4 != 0
        observedBacklog = if (value.detail and 8 != 0) KagemushaWalletUInt128V1(value.sequenceLow, value.sequenceHigh) else null
    }
}

/** Exact retained online claim originals; neither delivery nor payout acknowledgement. */
class KagemushaWalletFeeClaimV1 internal constructor(payment: ByteArray, request: ByteArray) {
    private val retainedPayment = payment.copyOf()
    private val retainedRequest = request.copyOf()
    fun payment(): ByteArray = retainedPayment.copyOf()
    fun request(): ByteArray = retainedRequest.copyOf()
    override fun toString(): String = "KagemushaWalletFeeClaimV1(originals=[REDACTED])"
}
/** Last durably selected native Global-chain decision; no payout permission is implied. */
class KagemushaWalletLedgerProgressV1 internal constructor(result: KagemushaWalletCallV1) {
    /** Unsigned u64 height carried as its exact Long bits. */
    val heightBits: Long
    private val hash: ByteArray
    init {
        if (result.status != 33) throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        heightBits = result.sequenceLow; hash = result.bytes()
    }
    fun blockHash(): ByteArray = hash.copyOf()
}
