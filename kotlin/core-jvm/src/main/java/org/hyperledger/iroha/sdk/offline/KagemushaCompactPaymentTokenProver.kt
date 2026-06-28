package org.hyperledger.iroha.sdk.offline

/** Native record-backed Kagemusha compact payment token prover. */
class KagemushaCompactPaymentTokenProver private constructor() {
    companion object {
        private const val LIBRARY_NAME = "connect_norito_bridge"
        const val NATIVE_ARCHIVE_MAX_BYTES: Int = 64 * 1024 * 1024
        private const val NORITO_HEADER_BYTES: Int = 40
        private const val NORITO_MAX_HEADER_PADDING_BYTES: Int = 64
        private const val NORITO_SUPPORTED_FLAGS_MASK: Int = 0x27
        private const val NORITO_FIELD_BITSET_FLAG: Int = 0x20
        private const val NORITO_FIELD_BITSET_REQUIRED_FLAGS: Int = 0x06
        private const val CRC64_REFLECTED_POLY: Long = -3932672073523589310L
        private val NORITO_MAGIC = byteArrayOf(
            'N'.code.toByte(),
            'R'.code.toByte(),
            'T'.code.toByte(),
            '0'.code.toByte(),
        )
        private val CRC64_TABLE = buildCrc64Table()
        private val nativeAvailable: Boolean = loadLibrary()

        @JvmStatic
        fun isNativeAvailable(): Boolean = nativeAvailable

        @JvmStatic
        fun proveVerifiedCompactPaymentTokenWithRecords(recordBundleArchive: ByteArray?): ByteArray {
            val recordBundle = ownedNativeInput(recordBundleArchive, "recordBundleArchive")
            check(nativeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
            val tokenArchive = nativeProveVerifiedCompactPaymentTokenWithRecords(recordBundle)
            return requireNativeOutput(
                tokenArchive,
                "nativeProveVerifiedCompactPaymentTokenWithRecords",
            )
        }

        internal fun ownedNativeInput(archiveInput: ByteArray?, archiveName: String): ByteArray {
            val archive = requireNativeInput(archiveInput, archiveName)
            return archive.copyOf()
        }

        internal fun requireNativeInput(archive: ByteArray?, archiveName: String): ByteArray {
            require(archive != null && archive.isNotEmpty()) { "$archiveName must not be empty" }
            require(archive.size <= NATIVE_ARCHIVE_MAX_BYTES) {
                "$archiveName must not exceed $NATIVE_ARCHIVE_MAX_BYTES bytes"
            }
            require(isValidNoritoArchive(archive)) {
                "$archiveName must be a valid Norito archive"
            }
            require(hasNonEmptyNoritoPayload(archive)) {
                "$archiveName must contain a non-empty Norito payload"
            }
            return archive
        }

        private fun loadLibrary(): Boolean =
            detectNativeAvailability(
                loadLibrary = { System.loadLibrary(LIBRARY_NAME) },
                probeSymbol = {
                    expectIllegalArgumentProbe {
                        nativeProveVerifiedCompactPaymentTokenWithRecords(ByteArray(0))
                    }
                },
            )

        internal fun detectNativeAvailability(loadLibrary: () -> Unit, probeSymbol: () -> Boolean): Boolean {
            try {
                loadLibrary()
            } catch (_: IllegalArgumentException) {
                return false
            } catch (_: UnsatisfiedLinkError) {
                return false
            } catch (_: SecurityException) {
                return false
            } catch (_: RuntimeException) {
                return false
            }
            return try {
                probeSymbol()
            } catch (_: IllegalArgumentException) {
                false
            } catch (_: UnsatisfiedLinkError) {
                false
            } catch (_: SecurityException) {
                false
            } catch (_: RuntimeException) {
                false
            }
        }

        internal fun expectIllegalArgumentProbe(probe: () -> Unit): Boolean =
            try {
                probe()
                false
            } catch (_: IllegalArgumentException) {
                true
            }

        internal fun requireNativeOutput(output: ByteArray?, label: String): ByteArray {
            check(output != null) { "$label returned no output" }
            check(output.isNotEmpty()) { "$label returned empty output" }
            check(output.size <= NATIVE_ARCHIVE_MAX_BYTES) { "$label returned oversized output" }
            check(isValidNoritoArchive(output)) { "$label returned invalid Norito archive" }
            check(hasNonEmptyNoritoPayload(output)) { "$label returned empty Norito payload" }
            return output
        }

        internal fun isValidNoritoArchive(output: ByteArray?): Boolean {
            if (output == null ||
                output.size < NORITO_HEADER_BYTES ||
                output.size > NATIVE_ARCHIVE_MAX_BYTES
            ) {
                return false
            }
            for (index in NORITO_MAGIC.indices) {
                if (output[index] != NORITO_MAGIC[index]) {
                    return false
                }
            }
            if (output[4].toInt() != 0 || output[5].toInt() != 0 || output[22].toInt() != 0) {
                return false
            }
            val flags = output[39].toInt() and 0xff
            if ((flags and NORITO_SUPPORTED_FLAGS_MASK.inv()) != 0) {
                return false
            }
            if ((flags and NORITO_FIELD_BITSET_FLAG) != 0 &&
                (flags and NORITO_FIELD_BITSET_REQUIRED_FLAGS) != NORITO_FIELD_BITSET_REQUIRED_FLAGS
            ) {
                return false
            }
            val payloadLengthLong = readLongLittleEndian(output, 23)
            if (payloadLengthLong < 0 ||
                payloadLengthLong > Int.MAX_VALUE.toLong() - NORITO_HEADER_BYTES
            ) {
                return false
            }
            val payloadLength = payloadLengthLong.toInt()
            val minimumLength = NORITO_HEADER_BYTES + payloadLength
            if (output.size < minimumLength) {
                return false
            }
            val paddingLength = output.size - minimumLength
            if (paddingLength > NORITO_MAX_HEADER_PADDING_BYTES) {
                return false
            }
            for (index in NORITO_HEADER_BYTES until NORITO_HEADER_BYTES + paddingLength) {
                if (output[index].toInt() != 0) {
                    return false
                }
            }
            val payloadOffset = NORITO_HEADER_BYTES + paddingLength
            val expectedCrc = readLongLittleEndian(output, 31)
            return crc64(output, payloadOffset, output.size - payloadOffset) == expectedCrc
        }

        internal fun hasNonEmptyNoritoPayload(output: ByteArray?): Boolean =
            output != null && isValidNoritoArchive(output) && readLongLittleEndian(output, 23) > 0

        private fun buildCrc64Table(): LongArray {
            val table = LongArray(256)
            for (index in table.indices) {
                var crc = index.toLong()
                for (bit in 0 until 8) {
                    crc = if ((crc and 1L) != 0L) {
                        (crc ushr 1) xor CRC64_REFLECTED_POLY
                    } else {
                        crc ushr 1
                    }
                }
                table[index] = crc
            }
            return table
        }

        private fun crc64(output: ByteArray, offset: Int, length: Int): Long {
            var crc = -1L
            for (index in offset until offset + length) {
                crc = CRC64_TABLE[
                    (crc.toInt() xor output[index].toInt()) and 0xff
                ] xor (crc ushr 8)
            }
            return crc xor -1L
        }

        private fun readLongLittleEndian(output: ByteArray, offset: Int): Long {
            var value = 0L
            for (index in 0 until 8) {
                value = value or ((output[offset + index].toLong() and 0xffL) shl (8 * index))
            }
            return value
        }

        @JvmStatic
        private external fun nativeProveVerifiedCompactPaymentTokenWithRecords(
            recordBundleArchive: ByteArray,
        ): ByteArray?
    }
}
