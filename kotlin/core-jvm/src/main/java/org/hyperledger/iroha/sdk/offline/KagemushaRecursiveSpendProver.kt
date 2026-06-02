package org.hyperledger.iroha.sdk.offline

/** Native recursive Kagemusha spend init/append/verify/redeem bridge. */
class KagemushaRecursiveSpendProver private constructor() {
    enum class Mode(val wireName: String) {
        RECURSIVE_SPEND_V1("recursive_spend_v1"),
        CHECKED_PREFOLD_V1("checked_prefold_v1"),
    }

    companion object {
        const val REQUIRED_BRIDGE_ABI_VERSION: Int = 6
        const val RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 =
            "kagemusha-recursive-aggregation-v1"
        const val RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1 =
            "kagemusha-recursive-spend-lineage-v1"

        private const val LIBRARY_NAME = "connect_norito_bridge"
        private val nativeAvailable: Boolean = loadLibrary()
        @JvmStatic
        fun isNativeAvailable(): Boolean = nativeAvailable

        @JvmStatic
        fun preferredMode(): Mode = preferredMode(nativeAvailable)

        @JvmStatic
        fun preferredMode(recursiveSpendAvailable: Boolean): Mode =
            if (recursiveSpendAvailable) Mode.RECURSIVE_SPEND_V1 else Mode.CHECKED_PREFOLD_V1

        @JvmStatic
        fun initSpend(requestArchive: ByteArray): ByteArray =
            call("init", requestArchive, ::nativeInitSpend)

        @JvmStatic
        fun appendSpend(requestArchive: ByteArray): ByteArray =
            call("append", requestArchive, ::nativeAppendSpend)

        @JvmStatic
        fun lineageWitnessFromInitResult(
            requestArchive: ByteArray,
            bundleArchive: ByteArray,
        ): ByteArray =
            call(
                "lineage witness from init result",
                requestArchive,
                bundleArchive,
                ::nativeLineageWitnessFromInitResult,
            )

        @JvmStatic
        fun lineageWitnessAppendResult(
            previousWitnessArchive: ByteArray,
            requestArchive: ByteArray,
            bundleArchive: ByteArray,
        ): ByteArray =
            call(
                "lineage witness append result",
                previousWitnessArchive,
                requestArchive,
                bundleArchive,
                ::nativeLineageWitnessAppendResult,
            )

        @JvmStatic
        fun verifySpend(requestArchive: ByteArray): ByteArray =
            call("verify", requestArchive, ::nativeVerifySpend)

        @JvmStatic
        fun redeemSpend(requestArchive: ByteArray): ByteArray =
            call("redeem", requestArchive, ::nativeRedeemSpend)

        private fun call(
            label: String,
            requestArchive: ByteArray,
            nativeCall: (ByteArray) -> ByteArray?,
        ): ByteArray {
            require(requestArchive.isNotEmpty()) { "requestArchive must not be empty" }
            check(nativeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
            val output = nativeCall(requestArchive)
            return KagemushaCompactPaymentTokenProver.requireNativeOutput(output, "native $label")
        }

        private fun call(
            label: String,
            requestArchive: ByteArray,
            bundleArchive: ByteArray,
            nativeCall: (ByteArray, ByteArray) -> ByteArray?,
        ): ByteArray {
            require(requestArchive.isNotEmpty()) { "requestArchive must not be empty" }
            require(bundleArchive.isNotEmpty()) { "bundleArchive must not be empty" }
            check(nativeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
            val output = nativeCall(requestArchive, bundleArchive)
            return KagemushaCompactPaymentTokenProver.requireNativeOutput(output, "native $label")
        }

        private fun call(
            label: String,
            previousWitnessArchive: ByteArray,
            requestArchive: ByteArray,
            bundleArchive: ByteArray,
            nativeCall: (ByteArray, ByteArray, ByteArray) -> ByteArray?,
        ): ByteArray {
            require(previousWitnessArchive.isNotEmpty()) { "previousWitnessArchive must not be empty" }
            require(requestArchive.isNotEmpty()) { "requestArchive must not be empty" }
            require(bundleArchive.isNotEmpty()) { "bundleArchive must not be empty" }
            check(nativeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
            val output = nativeCall(previousWitnessArchive, requestArchive, bundleArchive)
            return KagemushaCompactPaymentTokenProver.requireNativeOutput(output, "native $label")
        }

        private fun loadLibrary(): Boolean =
            detectNativeAvailability(
                loadLibrary = { System.loadLibrary(LIBRARY_NAME) },
                bridgeAbiVersion = { nativeBridgeAbiVersion() },
                probeSymbol = { probeRequiredNativeSymbols() },
            )

        private fun probeRequiredNativeSymbols(): Boolean {
            var available = true
            available = expectIllegalArgumentProbe { nativeInitSpend(ByteArray(0)) } && available
            available = expectIllegalArgumentProbe { nativeAppendSpend(ByteArray(0)) } && available
            available = expectIllegalArgumentProbe { nativeVerifySpend(ByteArray(0)) } && available
            available = expectIllegalArgumentProbe {
                nativeLineageWitnessFromInitResult(ByteArray(0), byteArrayOf(0x01))
            } && available
            available = expectIllegalArgumentProbe {
                nativeLineageWitnessAppendResult(ByteArray(0), byteArrayOf(0x01), byteArrayOf(0x02))
            } && available
            available = expectIllegalArgumentProbe { nativeRedeemSpend(ByteArray(0)) } && available
            return available
        }

        internal fun expectIllegalArgumentProbe(probe: () -> Unit): Boolean =
            try {
                probe()
                false
            } catch (_: IllegalArgumentException) {
                true
            }

        internal fun detectNativeAvailability(
            loadLibrary: () -> Unit,
            bridgeAbiVersion: () -> Int,
            probeSymbol: () -> Boolean,
        ): Boolean {
            try {
                loadLibrary()
            } catch (_: IllegalArgumentException) {
                return false
            } catch (_: UnsatisfiedLinkError) {
                return false
            } catch (_: SecurityException) {
                return false
            }
            val abiVersion = try {
                bridgeAbiVersion()
            } catch (_: IllegalArgumentException) {
                return false
            } catch (_: UnsatisfiedLinkError) {
                return false
            } catch (_: SecurityException) {
                return false
            }
            if (abiVersion < REQUIRED_BRIDGE_ABI_VERSION) {
                return false
            }
            return try {
                probeSymbol()
            } catch (_: UnsatisfiedLinkError) {
                false
            } catch (_: SecurityException) {
                false
            } catch (_: IllegalArgumentException) {
                false
            }
        }

        @JvmStatic
        private external fun nativeBridgeAbiVersion(): Int

        @JvmStatic
        private external fun nativeInitSpend(requestArchive: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativeAppendSpend(requestArchive: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativeLineageWitnessFromInitResult(
            requestArchive: ByteArray,
            bundleArchive: ByteArray,
        ): ByteArray?

        @JvmStatic
        private external fun nativeLineageWitnessAppendResult(
            previousWitnessArchive: ByteArray,
            requestArchive: ByteArray,
            bundleArchive: ByteArray,
        ): ByteArray?

        @JvmStatic
        private external fun nativeVerifySpend(requestArchive: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativeRedeemSpend(requestArchive: ByteArray): ByteArray?
    }
}
