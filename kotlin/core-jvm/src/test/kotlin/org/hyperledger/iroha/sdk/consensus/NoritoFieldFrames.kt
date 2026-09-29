package org.hyperledger.iroha.sdk.consensus

import java.io.ByteArrayOutputStream

/**
 * Minimal Norito struct-field framing for negative wire tests: each field is a
 * canonical unsigned LEB128 length followed by exactly that many bytes.
 */
internal object NoritoFieldFrames {
    private const val MAX_FIELD_BYTES = 32 * 1024 * 1024

    /** Returns the canonical (shortest) LEB128 encoding of [value]. */
    fun length(value: Int): ByteArray {
        require(value >= 0)
        var n = value
        val out = ByteArrayOutputStream()
        do { val b = n and 127; n = n ushr 7; out.write(b or if (n == 0) 0 else 128) } while (n != 0)
        return out.toByteArray()
    }

    /** Frames every field with its canonical length prefix, in order. */
    fun record(vararg fields: ByteArray): ByteArray {
        val out = ByteArrayOutputStream()
        fields.forEach { out.write(length(it.size)); out.write(it) }
        return out.toByteArray()
    }

    /** Strict cursor over length-framed fields; non-canonical prefixes and overruns fail. */
    class Reader(private val bytes: ByteArray) {
        private var cursor = 0
        private fun remaining() = bytes.size - cursor

        /** Takes exactly [count] unframed bytes. */
        fun raw(count: Int): ByteArray {
            require(count >= 0 && count <= remaining())
            return bytes.copyOfRange(cursor, cursor + count).also { cursor += count }
        }

        /** Requires that every byte was consumed. */
        fun finish() = require(remaining() == 0) { "trailing field bytes" }

        /** Takes one canonical length-framed field. */
        fun field(): ByteArray {
            var value = 0; var shift = 0; var count = 0
            while (true) {
                require(shift <= 28)
                val b = raw(1)[0].toInt() and 255
                require(shift < 28 || b <= 7)
                value = value or ((b and 127) shl shift); count++
                if (b and 128 == 0) break
                shift += 7
            }
            require(value <= MAX_FIELD_BYTES && length(value).size == count)
            return raw(value)
        }
    }
}
