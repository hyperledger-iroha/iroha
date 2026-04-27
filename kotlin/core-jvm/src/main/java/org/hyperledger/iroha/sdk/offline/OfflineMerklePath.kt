package org.hyperledger.iroha.sdk.offline

/** Direction bits and sibling hashes for one Merkle inclusion path. */
class OfflineMerklePath(
    dirs: ByteArray,
    siblings: List<ByteArray>,
) {
    private val _dirs: ByteArray = dirs.copyOf()
    private val _siblings: List<ByteArray> = siblings.map { it.copyOf() }

    val dirs: ByteArray get() = _dirs.copyOf()
    val siblings: List<ByteArray> get() = _siblings.map { it.copyOf() }

    internal fun toJsonMap(): Map<String, Any?> {
        val map = LinkedHashMap<String, Any?>()
        map["dirs"] = encodeBytes(_dirs)
        map["siblings"] = _siblings.map { encodeBytesAsHex(it) }
        return map
    }

    internal companion object {
        @JvmStatic
        fun encodeBytes(bytes: ByteArray): List<Int> = bytes.map { it.toInt() and 0xff }

        @JvmStatic
        fun encodeBytesAsHex(bytes: ByteArray): String {
            val hex = StringBuilder(bytes.size * 2)
            for (b in bytes) {
                val v = b.toInt() and 0xff
                hex.append(HEX_DIGITS[v ushr 4])
                hex.append(HEX_DIGITS[v and 0x0f])
            }
            return hex.toString()
        }

        private val HEX_DIGITS = "0123456789abcdef".toCharArray()
    }
}
