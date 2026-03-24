package org.hyperledger.iroha.sdk.offline

/** Commitment payload for an offline allowance. */
class OfflineAllowanceCommitment(
    val assetId: String,
    val amount: String,
    commitment: ByteArray,
) {
    private val _commitment: ByteArray = commitment.copyOf()

    val commitment: ByteArray get() = _commitment.copyOf()

    internal fun toJsonMap(): Map<String, Any> {
        val map = LinkedHashMap<String, Any>()
        map["asset"] = assetId
        map["amount"] = amount
        map["commitment"] = encodeBytes(_commitment)
        return map
    }

    internal companion object {
        @JvmStatic
        fun encodeBytes(bytes: ByteArray): List<Int> =
            bytes.map { it.toInt() and 0xff }
    }
}
