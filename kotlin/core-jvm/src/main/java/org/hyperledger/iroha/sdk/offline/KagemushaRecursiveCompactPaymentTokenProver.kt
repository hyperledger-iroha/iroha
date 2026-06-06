package org.hyperledger.iroha.sdk.offline

/** Native ABI-7 Kagemusha recursive compact-token prover and verifier. */
class KagemushaRecursiveCompactPaymentTokenProver private constructor() {
    companion object {
        private const val LIBRARY_NAME = "connect_norito_bridge"
        const val REQUIRED_BRIDGE_ABI_VERSION: Int = 7
        const val RECURSIVE_COMPACT_CIRCUIT_ID_V1: String =
            "kagemusha-recursive-compact-v1"
        private val nativeAvailable: Boolean = loadLibrary()

        @JvmStatic
        fun isNativeAvailable(): Boolean = nativeAvailable

        @JvmStatic
        fun proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
            recordBundleArchive: ByteArray,
            pallasOpenEnvelopesArchive: ByteArray,
        ): ByteArray {
            requireNativeInput(recordBundleArchive, "recordBundleArchive")
            requireNativeInput(pallasOpenEnvelopesArchive, "pallasOpenEnvelopesArchive")
            check(nativeAvailable) {
                "$LIBRARY_NAME ABI 7 recursive compact-token prover/verifier is not available in this runtime"
            }
            val tokenArchive =
                nativeProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive,
                    pallasOpenEnvelopesArchive,
                )
            return KagemushaCompactPaymentTokenProver.requireNativeOutput(
                tokenArchive,
                "nativeProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes",
            )
        }

        private fun requireNativeInput(archive: ByteArray, archiveName: String) {
            require(archive.isNotEmpty()) { "$archiveName must not be empty" }
            require(KagemushaCompactPaymentTokenProver.isValidNoritoArchive(archive)) {
                "$archiveName must be a valid Norito archive"
            }
            require(KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload(archive)) {
                "$archiveName must contain a non-empty Norito payload"
            }
        }

        @JvmStatic
        fun verifyRecursiveCompactPaymentToken(compactTokenArchive: ByteArray): Boolean {
            require(compactTokenArchive.isNotEmpty()) { "compactTokenArchive must not be empty" }
            require(KagemushaCompactPaymentTokenProver.isValidNoritoArchive(compactTokenArchive)) {
                "compactTokenArchive must be a valid Norito archive"
            }
            require(KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload(compactTokenArchive)) {
                "compactTokenArchive must contain a non-empty Norito payload"
            }
            check(nativeAvailable) {
                "$LIBRARY_NAME ABI 7 recursive compact-token verifier is not available in this runtime"
            }
            return nativeVerifyRecursiveCompactPaymentToken(compactTokenArchive)
        }

        private fun loadLibrary(): Boolean =
            KagemushaRecursiveSpendProver.detectNativeAvailability(
                loadLibrary = { System.loadLibrary(LIBRARY_NAME) },
                bridgeAbiVersion = { nativeBridgeAbiVersion() },
                probeSymbol = {
                    val proverRejects = KagemushaCompactPaymentTokenProver.expectIllegalArgumentProbe {
                        nativeProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                            ByteArray(0),
                            ByteArray(0),
                        )
                    }
                    val verifierRejects = KagemushaCompactPaymentTokenProver.expectIllegalArgumentProbe {
                        nativeVerifyRecursiveCompactPaymentToken(ByteArray(0))
                    }
                    proverRejects && verifierRejects
                },
                requiredBridgeAbiVersion = REQUIRED_BRIDGE_ABI_VERSION,
            )

        @JvmStatic
        private external fun nativeBridgeAbiVersion(): Int

        @JvmStatic
        private external fun nativeProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
            recordBundleArchive: ByteArray,
            pallasOpenEnvelopesArchive: ByteArray,
        ): ByteArray?

        @JvmStatic
        private external fun nativeVerifyRecursiveCompactPaymentToken(
            compactTokenArchive: ByteArray,
        ): Boolean
    }
}
