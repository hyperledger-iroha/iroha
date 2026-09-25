// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.wallet

/** A native Experimental startup rejected a checkpoint or lacks its independent context. */
class KagemushaTestnetNativeStartupExceptionV1(val nativeStatus: Int) : IllegalStateException(
    if (nativeStatus == -312) "KAGEMUSHA testnet native startup context is unavailable"
    else "KAGEMUSHA testnet native startup rejected: $nativeStatus",
)

internal interface KagemushaTestnetNativeStartupEndpointV1 {
    fun contract(): IntArray?
    fun activate(signedBootstrap: ByteArray): Int
}

/**
 * Activates the durable Experimental owner from a threshold-signed deployment checkpoint.
 * Rust independently supplies policy, deployment pins, trusted time, replay floor, signed
 * release and storage. Success neither credits a top-up nor qualifies offline spending.
 * An uncertain native installation requires recovery in a new process.
 */
class KagemushaTestnetNativeStartupV1 private constructor(
    private val endpoint: KagemushaTestnetNativeStartupEndpointV1,
) {
    /** Transport only the signed package; every retry is authenticated again inside Rust. */
    fun activate(signedBootstrap: ByteArray) {
        require(signedBootstrap.size in 1..MAXIMUM_BOOTSTRAP_BYTES) {
            "KAGEMUSHA signed bootstrap length is invalid"
        }
        val status = try {
            endpoint.activate(signedBootstrap.copyOf())
        } catch (error: LinkageError) {
            throw IllegalStateException("KAGEMUSHA testnet startup JNI is unavailable", error)
        }
        if (status != 0) throw KagemushaTestnetNativeStartupExceptionV1(status)
    }

    companion object {
        const val MAXIMUM_BOOTSTRAP_BYTES: Int = 1024 * 1024
        private val EXPECTED_CONTRACT = intArrayOf(1, MAXIMUM_BOOTSTRAP_BYTES)

        /** Load and require the exact native startup contract before exposing activation. */
        @JvmStatic
        fun open(): KagemushaTestnetNativeStartupV1 {
            try {
                System.loadLibrary("connect_norito_bridge")
                return openEndpoint(KagemushaTestnetNativeStartupJniV1)
            } catch (error: LinkageError) {
                throw IllegalStateException("KAGEMUSHA testnet startup JNI is unavailable", error)
            }
        }

        internal fun openEndpoint(
            endpoint: KagemushaTestnetNativeStartupEndpointV1,
        ): KagemushaTestnetNativeStartupV1 {
            val contract = try {
                endpoint.contract()
            } catch (error: LinkageError) {
                throw IllegalStateException("KAGEMUSHA testnet startup JNI is unavailable", error)
            }
            check(contract?.contentEquals(EXPECTED_CONTRACT) == true) {
                "KAGEMUSHA testnet startup JNI contract mismatch"
            }
            return KagemushaTestnetNativeStartupV1(endpoint)
        }
    }
}

/** Native owns authentication and installation; Kotlin has no replacement verifier. */
internal object KagemushaTestnetNativeStartupJniV1 : KagemushaTestnetNativeStartupEndpointV1 {
    override fun contract(): IntArray? = nativeContractV1()
    override fun activate(signedBootstrap: ByteArray): Int = nativeActivateV1(signedBootstrap)

    @JvmStatic private external fun nativeContractV1(): IntArray?
    @JvmStatic private external fun nativeActivateV1(signedBootstrap: ByteArray): Int
}
