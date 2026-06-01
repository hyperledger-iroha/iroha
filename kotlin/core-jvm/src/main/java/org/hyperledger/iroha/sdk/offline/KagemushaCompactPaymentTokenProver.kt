package org.hyperledger.iroha.sdk.offline

/** Native record-backed Kagemusha compact payment token prover. */
class KagemushaCompactPaymentTokenProver private constructor() {
    companion object {
        private const val LIBRARY_NAME = "connect_norito_bridge"
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
                probeSymbol = { nativeProveVerifiedCompactPaymentTokenWithRecords(ByteArray(0)) },
            )

        internal fun detectNativeAvailability(loadLibrary: () -> Unit, probeSymbol: () -> Unit): Boolean =
            try {
                loadLibrary()
                probeSymbol()
                true
            } catch (_: IllegalArgumentException) {
                true
            } catch (_: UnsatisfiedLinkError) {
                false
            } catch (_: SecurityException) {
                false
            }

        internal fun requireNativeOutput(output: ByteArray?, label: String): ByteArray {
            check(output != null) { "$label returned no output" }
            check(output.isNotEmpty()) { "$label returned empty output" }
            return output
        }

        @JvmStatic
        private external fun nativeProveVerifiedCompactPaymentTokenWithRecords(
            recordBundleArchive: ByteArray,
        ): ByteArray?
    }
}
