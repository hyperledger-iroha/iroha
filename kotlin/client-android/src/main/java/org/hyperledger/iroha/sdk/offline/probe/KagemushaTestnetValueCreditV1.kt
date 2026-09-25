// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.probe

import java.nio.ByteBuffer

/** Exact JNI transport for one credit already committed by the native testnet ledger. */
internal interface KagemushaTestnetValueCreditEndpointV1 {
    fun contract(): IntArray?
    fun credit(operationId: ByteArray, output: ByteBuffer): Int
}

/** Inspect one Experimental mint credit by operation ID after durable native admission. */
class KagemushaTestnetValueCreditV1 private constructor(
    private val endpoint: KagemushaTestnetValueCreditEndpointV1,
) {
    /** Return a copyable canonical credit archive; it grants no production or spend authority. */
    fun creditFinalizedValue(operationId: ByteArray): ByteArray {
        require(operationId.size == OPERATION_ID_BYTES && operationId.any { it != 0.toByte() }) {
            "KAGEMUSHA testnet credit operation ID is invalid"
        }
        val output = ByteBuffer.allocateDirect(ARCHIVE_MAX_BYTES)
        val status = try {
            endpoint.credit(operationId.copyOf(), output)
        } catch (error: LinkageError) {
            throw IllegalStateException("KAGEMUSHA testnet credit JNI is unavailable", error)
        }
        if (status !in 1..ARCHIVE_MAX_BYTES) {
            val message = when (status) {
                -312 -> "KAGEMUSHA durable testnet value owner is unavailable"
                -311 -> "KAGEMUSHA testnet value credit was rejected"
                else -> "KAGEMUSHA testnet value credit failed: $status"
            }
            throw KagemushaTestnetObservationExceptionV1(status, message)
        }
        val archive = ByteArray(status)
        output.position(0)
        output.get(archive)
        return archive
    }

    companion object {
        private const val OPERATION_ID_BYTES = 32
        private const val ARCHIVE_MAX_BYTES = 512
        private val EXPECTED_CONTRACT = intArrayOf(1, OPERATION_ID_BYTES, ARCHIVE_MAX_BYTES)

        /** Open only the exact source-matched native contract. */
        @JvmStatic
        fun open(): KagemushaTestnetValueCreditV1 {
            try {
                System.loadLibrary("connect_norito_bridge")
                return openEndpoint(KagemushaTestnetValueCreditJniV1)
            } catch (error: LinkageError) {
                throw IllegalStateException("KAGEMUSHA testnet credit JNI is unavailable", error)
            }
        }

        internal fun openEndpoint(
            endpoint: KagemushaTestnetValueCreditEndpointV1,
        ): KagemushaTestnetValueCreditV1 {
            val contract = try {
                endpoint.contract()
            } catch (error: LinkageError) {
                throw IllegalStateException("KAGEMUSHA testnet credit JNI is unavailable", error)
            }
            check(contract?.contentEquals(EXPECTED_CONTRACT) == true) {
                "KAGEMUSHA testnet credit JNI contract mismatch"
            }
            return KagemushaTestnetValueCreditV1(endpoint)
        }
    }
}

/** Rust owns the ledger and canonical archive. Kotlin supplies no verification fallback. */
internal object KagemushaTestnetValueCreditJniV1 : KagemushaTestnetValueCreditEndpointV1 {
    override fun contract(): IntArray? = nativeContractV1()

    override fun credit(operationId: ByteArray, output: ByteBuffer): Int =
        nativeCreditV1(operationId, output)

    @JvmStatic private external fun nativeContractV1(): IntArray?
    @JvmStatic private external fun nativeCreditV1(operationId: ByteArray, output: ByteBuffer): Int
}
