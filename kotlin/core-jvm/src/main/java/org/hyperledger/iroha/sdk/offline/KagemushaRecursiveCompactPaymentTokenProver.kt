package org.hyperledger.iroha.sdk.offline

/** Native ABI-7 Kagemusha recursive compact-token prover and verifier. */
class KagemushaRecursiveCompactPaymentTokenProver private constructor() {
    companion object {
        private const val LIBRARY_NAME = "connect_norito_bridge"
        const val REQUIRED_BRIDGE_ABI_VERSION: Int = 7
        const val RECURSIVE_COMPACT_CIRCUIT_ID_V1: String =
            "kagemusha-recursive-compact-v1"
        private const val RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT =
            "recursive compact Kagemusha payment-token multi-hop proving requires the append verifier batch"
        private const val RECURSIVE_COMPACT_MULTI_HOP_UNAVAILABLE_FRAGMENT =
            "recursive compact Kagemusha multi-hop payment-token proving requires the append verifier batch"
        private val nativeVerifierAvailable: Boolean = loadVerifierLibrary()
        private val nativeProjectionVerifierAvailable: Boolean = loadProjectionVerifierLibrary()
        private val nativeAvailable: Boolean = loadLibrary()

        @JvmStatic
        fun isNativeAvailable(): Boolean = nativeAvailable

        @JvmStatic
        fun isVerifierNativeAvailable(): Boolean = nativeVerifierAvailable

        @JvmStatic
        fun isProjectionVerifierNativeAvailable(): Boolean = nativeProjectionVerifierAvailable

        @JvmStatic
        fun proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
            recordBundleArchive: ByteArray?,
            pallasOpenEnvelopesArchive: ByteArray?,
        ): ByteArray {
            requireNativeInput(recordBundleArchive, "recordBundleArchive")
            requireNativeInput(pallasOpenEnvelopesArchive, "pallasOpenEnvelopesArchive")
            val recordBundle = ownedNativeInput(recordBundleArchive, "recordBundleArchive")
            val pallasOpenEnvelopes = ownedNativeInput(pallasOpenEnvelopesArchive, "pallasOpenEnvelopesArchive")
            check(nativeAvailable) {
                "$LIBRARY_NAME ABI 7 recursive compact-token prover/verifier is not available in this runtime"
            }
            val tokenArchive =
                try {
                    nativeProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                        recordBundle,
                        pallasOpenEnvelopes,
                    )
                } catch (error: IllegalArgumentException) {
                    if (isRecursiveCompactUnavailable(error)) {
                        throw IllegalStateException(
                            "Kagemusha recursive compact proof composition is unavailable: ${error.message}",
                            error,
                        )
                    }
                    throw error
                }
            return KagemushaCompactPaymentTokenProver.requireNativeOutput(
                tokenArchive,
                "nativeProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes",
            )
        }

        @JvmStatic
        fun recursiveSpendCompactPaymentTokenFromBundle(
            bundleArchive: ByteArray?,
        ): ByteArray {
            val bundle = ownedNativeInput(bundleArchive, "bundleArchive")
            check(nativeAvailable) {
                "$LIBRARY_NAME ABI 7 recursive compact-token projection is not available in this runtime"
            }
            val tokenArchive = nativeRecursiveSpendCompactPaymentTokenFromBundle(bundle)
            return KagemushaCompactPaymentTokenProver.requireNativeOutput(
                tokenArchive,
                "nativeRecursiveSpendCompactPaymentTokenFromBundle",
            )
        }

        @JvmStatic
        fun isRecursiveCompactUnavailable(error: Throwable?): Boolean =
            isRecursiveCompactUnavailableMessage(error?.message)

        private fun isRecursiveCompactUnavailableMessage(message: String?): Boolean =
            message?.contains(RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT) == true ||
                message?.contains(RECURSIVE_COMPACT_MULTI_HOP_UNAVAILABLE_FRAGMENT) == true

        internal fun ownedNativeInput(archiveInput: ByteArray?, archiveName: String): ByteArray {
            val archive = requireNativeInput(archiveInput, archiveName)
            return archive.copyOf()
        }

        private fun requireNativeInput(archive: ByteArray?, archiveName: String): ByteArray {
            require(archive != null && archive.isNotEmpty()) { "$archiveName must not be empty" }
            require(archive.size <= KagemushaCompactPaymentTokenProver.NATIVE_ARCHIVE_MAX_BYTES) {
                "$archiveName must not exceed ${KagemushaCompactPaymentTokenProver.NATIVE_ARCHIVE_MAX_BYTES} bytes"
            }
            require(KagemushaCompactPaymentTokenProver.isValidNoritoArchive(archive)) {
                "$archiveName must be a valid Norito archive"
            }
            require(KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload(archive)) {
                "$archiveName must contain a non-empty Norito payload"
            }
            return archive
        }

        @JvmStatic
        fun verifyRecursiveCompactPaymentToken(compactTokenArchive: ByteArray?): Boolean {
            val compactToken = ownedNativeInput(compactTokenArchive, "compactTokenArchive")
            check(nativeVerifierAvailable) {
                "$LIBRARY_NAME ABI 7 recursive compact-token verifier is not available in this runtime"
            }
            return nativeVerifyRecursiveCompactPaymentToken(compactToken)
        }

        @JvmStatic
        fun verifyRecursiveSpendCompactPaymentTokenProjection(
            compactTokenArchive: ByteArray?,
            verifierRecordArchive: ByteArray?,
        ): Boolean {
            val compactToken = ownedNativeInput(compactTokenArchive, "compactTokenArchive")
            val verifierRecord = ownedNativeInput(verifierRecordArchive, "verifierRecordArchive")
            check(nativeProjectionVerifierAvailable) {
                "$LIBRARY_NAME ABI 7 recursive spend compact-token projection verifier is not available in this runtime"
            }
            return nativeVerifyRecursiveSpendCompactPaymentTokenProjection(compactToken, verifierRecord)
        }

        @JvmStatic
        fun verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
            compactTokenArchive: ByteArray?,
            verifierRecordArchive: ByteArray?,
            blockHeight: Long,
        ): Boolean {
            require(blockHeight >= 0) { "blockHeight must be non-negative" }
            val compactToken = ownedNativeInput(compactTokenArchive, "compactTokenArchive")
            val verifierRecord = ownedNativeInput(verifierRecordArchive, "verifierRecordArchive")
            check(nativeProjectionVerifierAvailable) {
                "$LIBRARY_NAME ABI 7 recursive spend compact-token projection verifier is not available in this runtime"
            }
            return nativeVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                compactToken,
                verifierRecord,
                blockHeight,
            )
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
                    val projectionRejects = KagemushaCompactPaymentTokenProver.expectIllegalArgumentProbe {
                        nativeRecursiveSpendCompactPaymentTokenFromBundle(ByteArray(0))
                    }
                    proverRejects && verifierRejects && projectionRejects
                },
                requiredBridgeAbiVersion = REQUIRED_BRIDGE_ABI_VERSION,
            )

        private fun loadVerifierLibrary(): Boolean =
            KagemushaRecursiveSpendProver.detectNativeAvailability(
                loadLibrary = { System.loadLibrary(LIBRARY_NAME) },
                bridgeAbiVersion = { nativeBridgeAbiVersion() },
                probeSymbol = {
                    KagemushaCompactPaymentTokenProver.expectIllegalArgumentProbe {
                        nativeVerifyRecursiveCompactPaymentToken(ByteArray(0))
                    }
                },
                requiredBridgeAbiVersion = REQUIRED_BRIDGE_ABI_VERSION,
            )

        private fun loadProjectionVerifierLibrary(): Boolean =
            KagemushaRecursiveSpendProver.detectNativeAvailability(
                loadLibrary = { System.loadLibrary(LIBRARY_NAME) },
                bridgeAbiVersion = { nativeBridgeAbiVersion() },
                probeSymbol = {
                    val noHeightRejects = KagemushaCompactPaymentTokenProver.expectIllegalArgumentProbe {
                        nativeVerifyRecursiveSpendCompactPaymentTokenProjection(ByteArray(0), ByteArray(0))
                    }
                    val heightRejects = KagemushaCompactPaymentTokenProver.expectIllegalArgumentProbe {
                        nativeVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                            ByteArray(0),
                            ByteArray(0),
                            0L,
                        )
                    }
                    noHeightRejects && heightRejects
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

        @JvmStatic
        private external fun nativeRecursiveSpendCompactPaymentTokenFromBundle(
            bundleArchive: ByteArray,
        ): ByteArray?

        @JvmStatic
        private external fun nativeVerifyRecursiveSpendCompactPaymentTokenProjection(
            compactTokenArchive: ByteArray,
            verifierRecordArchive: ByteArray,
        ): Boolean

        @JvmStatic
        private external fun nativeVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
            compactTokenArchive: ByteArray,
            verifierRecordArchive: ByteArray,
            blockHeight: Long,
        ): Boolean
    }
}
