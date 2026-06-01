package org.hyperledger.iroha.sdk.offline

/** Native recursive Kagemusha spend init/append/verify/redeem bridge. */
class KagemushaRecursiveSpendProver private constructor() {
    enum class Mode(val wireName: String) {
        RECURSIVE_SPEND_V1("recursive_spend_v1"),
        CHECKED_PREFOLD_V1("checked_prefold_v1"),
    }

    companion object {
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

        private fun loadLibrary(): Boolean =
            KagemushaRecursiveAggregationProofBundleProver.detectNativeAvailability(
                loadLibrary = { System.loadLibrary(LIBRARY_NAME) },
                probeSymbol = { nativeVerifySpend(ByteArray(0)) },
            )

        @JvmStatic
        private external fun nativeInitSpend(requestArchive: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativeAppendSpend(requestArchive: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativeVerifySpend(requestArchive: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativeRedeemSpend(requestArchive: ByteArray): ByteArray?
    }
}
