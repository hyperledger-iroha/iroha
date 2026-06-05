package org.hyperledger.iroha.sdk.offline

/** Native record-backed Kagemusha compact payment token prover. */
class KagemushaCompactPaymentTokenProver private constructor() {
    companion object {
        private const val LIBRARY_NAME = "connect_norito_bridge"
        const val NATIVE_ARCHIVE_MAX_BYTES: Int = 64 * 1024 * 1024
        private val nativeAvailable: Boolean = loadLibrary()

        @JvmStatic
        fun isNativeAvailable(): Boolean = nativeAvailable

        @JvmStatic
        fun proveVerifiedCompactPaymentTokenWithRecords(recordBundleArchive: ByteArray): ByteArray {
            require(recordBundleArchive.isNotEmpty()) { "recordBundleArchive must not be empty" }
            check(nativeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
            val tokenArchive = nativeProveVerifiedCompactPaymentTokenWithRecords(recordBundleArchive)
            return requireNativeOutput(
                tokenArchive,
                "nativeProveVerifiedCompactPaymentTokenWithRecords",
            )
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
            }
            return try {
                probeSymbol()
            } catch (_: IllegalArgumentException) {
                false
            } catch (_: UnsatisfiedLinkError) {
                false
            } catch (_: SecurityException) {
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
            return output
        }

        @JvmStatic
        private external fun nativeProveVerifiedCompactPaymentTokenWithRecords(
            recordBundleArchive: ByteArray,
        ): ByteArray?
    }
}
