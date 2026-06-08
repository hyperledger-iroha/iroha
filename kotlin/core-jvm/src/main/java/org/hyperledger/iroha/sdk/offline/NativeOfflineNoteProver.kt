package org.hyperledger.iroha.sdk.offline

/** Native record-backed Offline Note recursive prover using a chain-supplied verifying key. */
class NativeOfflineNoteProver private constructor() {
    companion object {
        private const val LIBRARY_NAME = "connect_norito_bridge"
        private val nativeAvailable: Boolean = loadLibrary()

        @JvmStatic
        fun isNativeAvailable(): Boolean = nativeAvailable

        @JvmStatic
        fun proveRedeem(redeemNorito: ByteArray, vkBoxNorito: ByteArray): ByteArray {
            require(redeemNorito.isNotEmpty()) { "redeemNorito must not be empty" }
            require(vkBoxNorito.isNotEmpty()) { "vkBoxNorito must not be empty" }
            check(nativeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
            val proofNorito = nativeProveNoteRedeemWithVk(redeemNorito, vkBoxNorito)
            check(proofNorito != null && proofNorito.isNotEmpty()) {
                "nativeProveNoteRedeemWithVk returned empty output"
            }
            return proofNorito
        }

        @JvmStatic
        fun proveAudit(auditNorito: ByteArray, vkBoxNorito: ByteArray): ByteArray {
            require(auditNorito.isNotEmpty()) { "auditNorito must not be empty" }
            require(vkBoxNorito.isNotEmpty()) { "vkBoxNorito must not be empty" }
            check(nativeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
            val proofNorito = nativeProveNoteAuditWithVk(auditNorito, vkBoxNorito)
            check(proofNorito != null && proofNorito.isNotEmpty()) {
                "nativeProveNoteAuditWithVk returned empty output"
            }
            return proofNorito
        }

        @JvmStatic
        fun verifyRedeem(redeemNorito: ByteArray, vkBoxNorito: ByteArray): Boolean {
            require(redeemNorito.isNotEmpty()) { "redeemNorito must not be empty" }
            require(vkBoxNorito.isNotEmpty()) { "vkBoxNorito must not be empty" }
            check(nativeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
            return nativeVerifyNoteRedeemWithVk(redeemNorito, vkBoxNorito)
        }

        @JvmStatic
        fun verifyAudit(auditNorito: ByteArray, vkBoxNorito: ByteArray): Boolean {
            require(auditNorito.isNotEmpty()) { "auditNorito must not be empty" }
            require(vkBoxNorito.isNotEmpty()) { "vkBoxNorito must not be empty" }
            check(nativeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
            return nativeVerifyNoteAuditWithVk(auditNorito, vkBoxNorito)
        }

        private fun loadLibrary(): Boolean =
            detectNativeAvailability(
                loadLibrary = { System.loadLibrary(LIBRARY_NAME) },
                probeSymbol = { nativeProveNoteRedeemWithVk(ByteArray(0), ByteArray(0)) },
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
        private external fun nativeProveNoteRedeemWithVk(
            redeemNorito: ByteArray,
            vkBoxNorito: ByteArray,
        ): ByteArray?

        @JvmStatic
        private external fun nativeProveNoteAuditWithVk(
            auditNorito: ByteArray,
            vkBoxNorito: ByteArray,
        ): ByteArray?

        @JvmStatic
        private external fun nativeVerifyNoteRedeemWithVk(
            redeemNorito: ByteArray,
            vkBoxNorito: ByteArray,
        ): Boolean

        @JvmStatic
        private external fun nativeVerifyNoteAuditWithVk(
            auditNorito: ByteArray,
            vkBoxNorito: ByteArray,
        ): Boolean
    }
}
