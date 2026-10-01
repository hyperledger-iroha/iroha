package org.hyperledger.iroha.sdk.crypto.keystore.attestation

import java.io.ByteArrayOutputStream

/** Original leaf-first Android attestation evidence, without certificate or device admission.
 * The independently selected verifier authenticates every certificate, root, revocation snapshot,
 * attested app identity and challenge. This archive only preserves the bounded original DER bytes.
 */
class KagemushaAndroidKeyAttestationArchiveV1 private constructor(raw: ByteArray, chain: List<ByteArray>) {
    private val original = raw.copyOf()
    private val certificates = chain.map(ByteArray::copyOf)
    fun transportBytes(): ByteArray = original.copyOf()
    fun certificateChainDer(): List<ByteArray> = certificates.map(ByteArray::copyOf)

    companion object {
        const val MAXIMUM_ARCHIVE_BYTES: Int = 128 * 1024
        const val MAXIMUM_CERTIFICATE_BYTES: Int = 16 * 1024
        private val MAGIC = byteArrayOf(0x4b, 0x4d, 0x43, 0x41, 1)

        /** Exact `KMCA\u0001 | count:u8 | repeated length:u32be + DER`, preserving leaf-first order. */
        @JvmStatic fun encodeOriginal(certificateChainDer: List<ByteArray>): KagemushaAndroidKeyAttestationArchiveV1 {
            require(certificateChainDer.size in 2..8) { "Android certificate count outside bound" }
            val chain = certificateChainDer.map(ByteArray::copyOf)
            val output = ByteArrayOutputStream()
            output.write(MAGIC); output.write(chain.size)
            for (der in chain) {
                requireSingleDerSequence(der)
                val length = der.size
                output.write(byteArrayOf((length ushr 24).toByte(), (length ushr 16).toByte(),
                    (length ushr 8).toByte(), length.toByte()))
                output.write(der)
                require(output.size() <= MAXIMUM_ARCHIVE_BYTES) { "Android attestation archive outside bound" }
            }
            return KagemushaAndroidKeyAttestationArchiveV1(output.toByteArray(), chain)
        }

        @JvmStatic fun parseOriginal(bytes: ByteArray): KagemushaAndroidKeyAttestationArchiveV1 {
            require(bytes.size in 6..MAXIMUM_ARCHIVE_BYTES) { "Android attestation archive outside bound" }
            val original = bytes.copyOf()
            require(original.copyOfRange(0, 5).contentEquals(MAGIC)) { "Android attestation archive version differs" }
            val count = original[5].toInt() and 0xff
            require(count in 2..8) { "Android certificate count outside bound" }
            val chain = ArrayList<ByteArray>(count)
            var offset = 6
            repeat(count) {
                require(offset + 4 <= original.size) { "Truncated Android certificate length" }
                val length = ((original[offset].toInt() and 0xff) shl 24) or
                    ((original[offset + 1].toInt() and 0xff) shl 16) or
                    ((original[offset + 2].toInt() and 0xff) shl 8) or (original[offset + 3].toInt() and 0xff)
                offset += 4
                require(length in 1..MAXIMUM_CERTIFICATE_BYTES && length <= original.size - offset) {
                    "Android DER certificate outside bound"
                }
                val der = original.copyOfRange(offset, offset + length)
                requireSingleDerSequence(der)
                chain += der; offset += length
            }
            require(offset == original.size) { "Trailing Android attestation archive bytes" }
            return KagemushaAndroidKeyAttestationArchiveV1(original, chain)
        }

        private fun requireSingleDerSequence(der: ByteArray) {
            require(der.size in 2..MAXIMUM_CERTIFICATE_BYTES && der[0] == 0x30.toByte()) { "Invalid DER certificate tag or size" }
            val first = der[1].toInt() and 0xff
            val header: Int
            val content: Int
            if (first < 128) { header = 2; content = first }
            else {
                val width = first and 0x7f
                require(width in 1..2 && der.size >= 2 + width && der[2] != 0.toByte()) { "Invalid DER certificate length" }
                var length = 0
                for (index in 0 until width) length = (length shl 8) or (der[2 + index].toInt() and 0xff)
                require(length >= 128 && (width != 2 || length > 0xff)) { "Nonminimal DER certificate length" }
                header = 2 + width; content = length
            }
            require(header + content == der.size) { "Trailing or truncated DER certificate" }
        }
    }
}
