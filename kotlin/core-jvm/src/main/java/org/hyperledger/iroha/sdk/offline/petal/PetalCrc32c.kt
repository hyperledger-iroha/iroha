package org.hyperledger.iroha.sdk.offline.petal

/**
 * CRC-32C (Castagnoli) that binds a reassembled Petal payload to its beacon.
 *
 * Reflected polynomial `0x82F63B78`, initial value `0xFFFFFFFF`, final xor
 * `0xFFFFFFFF`. Results are the raw 32-bit pattern in an [Int]; use
 * `Integer.toUnsignedLong` (Java) or `toUInt()` (Kotlin) for the unsigned value.
 */
object PetalCrc32c {
    private const val POLY = 0x82F6_3B78.toInt()

    private val TABLE = IntArray(256).also { table ->
        for (index in 0 until 256) {
            var crc = index
            repeat(8) { crc = if ((crc and 1) == 1) (crc ushr 1) xor POLY else crc ushr 1 }
            table[index] = crc
        }
    }

    /** CRC-32C of [bytes]. */
    @JvmStatic
    fun compute(bytes: ByteArray): Int = compute(bytes, 0, bytes.size)

    /** CRC-32C of `bytes[offset until offset + length]`. */
    @JvmStatic
    fun compute(bytes: ByteArray, offset: Int, length: Int): Int {
        require(offset >= 0 && length >= 0 && offset <= bytes.size - length) { "CRC-32C range out of bounds" }
        var crc = -1
        for (index in offset until offset + length) {
            crc = TABLE[(crc xor bytes[index].toInt()) and 0xFF] xor (crc ushr 8)
        }
        return crc.inv()
    }
}
