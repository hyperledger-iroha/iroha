package org.hyperledger.iroha.sdk.offline.wallet

/** Opaque single-use native exchange. No caller clock or decoder is available. */
class KagemushaWalletTimeExchangeV1 internal constructor(internal val owner: Long, internal val token: Long, nonce: ByteArray) {
    private val original = nonce.copyOf()
    /** Fresh native nonce to transmit to the issuer. */
    fun nonce(): ByteArray = original.copyOf()
}

/** Fixed intake and defensive copies only; Native authenticates and derives every field. */
internal class KagemushaWalletSetupInputV1(
    val selector: Int,
    identity: ByteArray = ByteArray(32),
    val amount: KagemushaWalletUInt128V1 = KagemushaWalletUInt128V1(0, 0),
    val token: Long = 0,
    first: ByteArray = byteArrayOf(), second: ByteArray = byteArrayOf(), third: ByteArray = byteArrayOf(),
) {
    private val id = identity.copyOf()
    private val a = first.copyOf()
    private val b = second.copyOf()
    private val c = third.copyOf()
    init {
        val limits = when (selector) {
            0, 1, 4, 6 -> intArrayOf(0, 0, 0)
            2 -> intArrayOf(10_000, 1_024, 512)
            3, in 7..14 -> intArrayOf(10_000, 0, 0)
            5 -> intArrayOf(512, 512, 0)
            else -> throw IllegalArgumentException("unknown setup action")
        }
        require(id.size == 32 && ((selector in 1..2) == id.any { it != 0.toByte() })) { "setup identity" }
        require((selector == 1) == (amount.low != 0L || amount.high != 0L)) { "Offer amount" }
        require(token >= 0 && ((selector == 5 || selector == 6) == (token != 0L))) { "native time token" }
        for ((bytes, bound) in listOf(a, b, c).zip(limits.toList())) require(bytes.size <= bound) { "setup input bound" }
        require(selector != 2 || (a.isNotEmpty() && b.isEmpty() == c.isEmpty())) { "Request originals" }
        require((selector != 3 && selector !in 7..14) || a.isNotEmpty()) { "Credited original" }
        require(selector != 5 || (a.isNotEmpty() && b.isNotEmpty())) { "time response originals" }
    }
    fun identity(): ByteArray = id.copyOf()
    fun first(): ByteArray = a.copyOf()
    fun second(): ByteArray = b.copyOf()
    fun third(): ByteArray = c.copyOf()
}

/** Exact peer-envelope kind; this cannot select proof keys or operation authority. */
enum class KagemushaWalletTransportKindV1(internal val tag: Int) {
    OFFER(1), REQUEST(2), PAYMENT(3), CREDITED(4),
}
