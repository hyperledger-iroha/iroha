package org.hyperledger.iroha.sdk.offline

/** Native record-backed Kagemusha recursive aggregation proof-bundle prover. */
class KagemushaRecursiveAggregationProofBundleProver private constructor() {
    companion object {
        private const val LIBRARY_NAME = "connect_norito_bridge"
        private val nativeAvailable: Boolean = loadLibrary()

        @JvmStatic
        fun isNativeAvailable(): Boolean = nativeAvailable

        @JvmStatic
        fun proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
            recordBundleArchive: ByteArray,
            pallasOpenEnvelopesArchive: ByteArray,
        ): ByteArray {
            require(recordBundleArchive.isNotEmpty()) { "recordBundleArchive must not be empty" }
            require(pallasOpenEnvelopesArchive.isNotEmpty()) {
                "pallasOpenEnvelopesArchive must not be empty"
            }
            check(nativeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
            val proofBundleArchive =
                nativeProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive,
                    pallasOpenEnvelopesArchive,
                )
            check(proofBundleArchive != null && proofBundleArchive.isNotEmpty()) {
                "nativeProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes returned empty output"
            }
            return proofBundleArchive
        }

        private fun loadLibrary(): Boolean =
            detectNativeAvailability(
                loadLibrary = { System.loadLibrary(LIBRARY_NAME) },
                probeSymbol = {
                    nativeProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                        ByteArray(0),
                        ByteArray(0),
                    )
                },
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

        @JvmStatic
        private external fun nativeProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
            recordBundleArchive: ByteArray,
            pallasOpenEnvelopesArchive: ByteArray,
        ): ByteArray?
    }
}
