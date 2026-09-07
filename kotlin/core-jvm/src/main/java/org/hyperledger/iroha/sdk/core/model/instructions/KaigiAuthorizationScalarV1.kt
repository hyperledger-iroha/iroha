package org.hyperledger.iroha.sdk.core.model.instructions

/** Exact public Pasta Fp output bytes; no Hash marker, reduction, or nonzero restriction. */
class KaigiAuthorizationScalarV1 private constructor(bytes: ByteArray) {
    private val encoded = bytes.copyOf()
    fun toLeBytes(): ByteArray = encoded.copyOf()
    fun toHex(): String = encoded.joinToString("") { "%02x".format(it.toInt() and 255) }
    override fun equals(other: Any?): Boolean =
        other is KaigiAuthorizationScalarV1 && encoded.contentEquals(other.encoded)
    override fun hashCode(): Int = encoded.contentHashCode()

    companion object {
        private val MODULUS = byteArrayOf(
            1, 0, 0, 0, 0xed.toByte(), 0x30, 0x2d, 0x99.toByte(),
            0x1b, 0xf9.toByte(), 0x4c, 9, 0xfc.toByte(), 0x98.toByte(), 0x46, 0x22,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x40,
        )

        /** Retain an exact 32-byte little-endian scalar strictly below the Pasta Fp modulus. */
        @JvmStatic
        fun fromLeBytes(bytes: ByteArray): KaigiAuthorizationScalarV1 {
            require(bytes.size == 32) { "Kaigi scalar must contain exactly 32 bytes" }
            val snapshot = bytes.copyOf()
            val different = (31 downTo 0).firstOrNull { snapshot[it] != MODULUS[it] }
            require(different != null &&
                (snapshot[different].toInt() and 255) < (MODULUS[different].toInt() and 255)) {
                "Kaigi scalar must be canonical Pasta Fp bytes below the modulus"
            }
            return KaigiAuthorizationScalarV1(snapshot)
        }

        internal fun fromArgument(value: String): KaigiAuthorizationScalarV1 {
            require(value.length == 64 && value.all { it in '0'..'9' || it in 'a'..'f' }) {
                "Kaigi scalar argument must be exactly 64 lowercase hexadecimal characters"
            }
            return fromLeBytes(ByteArray(32) { value.substring(it * 2, it * 2 + 2).toInt(16).toByte() })
        }
    }
}
