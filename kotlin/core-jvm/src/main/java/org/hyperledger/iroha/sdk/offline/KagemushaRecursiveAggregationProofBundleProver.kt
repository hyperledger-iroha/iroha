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
            recordBundleArchive: ByteArray?,
            pallasOpenEnvelopesArchive: ByteArray?,
        ): ByteArray {
            KagemushaCompactPaymentTokenProver.requireNativeInput(
                recordBundleArchive,
                "recordBundleArchive",
            )
            KagemushaCompactPaymentTokenProver.requireNativeInput(
                pallasOpenEnvelopesArchive,
                "pallasOpenEnvelopesArchive",
            )
            val recordBundle = KagemushaCompactPaymentTokenProver.ownedNativeInput(
                recordBundleArchive,
                "recordBundleArchive",
            )
            val pallasOpenEnvelopes = KagemushaCompactPaymentTokenProver.ownedNativeInput(
                pallasOpenEnvelopesArchive,
                "pallasOpenEnvelopesArchive",
            )
            check(nativeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
            val proofBundleArchive =
                nativeProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    recordBundle,
                    pallasOpenEnvelopes,
                )
            return KagemushaCompactPaymentTokenProver.requireNativeOutput(
                proofBundleArchive,
                "nativeProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes",
            )
        }

        private fun loadLibrary(): Boolean =
            detectNativeAvailability(
                loadLibrary = { System.loadLibrary(LIBRARY_NAME) },
                probeSymbol = {
                    expectIllegalArgumentProbe {
                        nativeProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                            ByteArray(0),
                            ByteArray(0),
                        )
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

        @JvmStatic
        private external fun nativeProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
            recordBundleArchive: ByteArray,
            pallasOpenEnvelopesArchive: ByteArray,
        ): ByteArray?
    }
}
