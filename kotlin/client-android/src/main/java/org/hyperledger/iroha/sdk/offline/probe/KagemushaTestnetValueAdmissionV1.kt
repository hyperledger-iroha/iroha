// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.probe

import java.nio.ByteBuffer

/** Exact JNI transport for a durable-owner-verified testnet mint value record. */
internal interface KagemushaTestnetValueAdmissionEndpointV1 {
    fun contract(): IntArray?
    fun admit(operationId: ByteArray, output: ByteBuffer): Int
}

/**
 * Inspect one experimental mint value already retained by the native durable owner.
 *
 * The returned canonical archive is copyable inspection data, not a spend credential or a
 * hardware attestation. Rust must have installed a signed Experimental release, persisted the
 * private reservation before submission, and independently pinned signed finality. This API
 * cannot create or replace any of those authorities.
 */
class KagemushaTestnetValueAdmissionV1 private constructor(
    private val endpoint: KagemushaTestnetValueAdmissionEndpointV1,
) {
    /** Obtain a bounded testnet value archive for an already verified Applied MintFold. */
    fun admitFinalizedValue(operationId: ByteArray): ByteArray {
        require(operationId.size == OPERATION_ID_BYTES && operationId.any { it != 0.toByte() }) {
            "KAGEMUSHA testnet value operation ID is invalid"
        }
        val output = ByteBuffer.allocateDirect(ARCHIVE_MAX_BYTES)
        val status = try {
            endpoint.admit(operationId.copyOf(), output)
        } catch (error: LinkageError) {
            throw IllegalStateException("KAGEMUSHA testnet value JNI is unavailable", error)
        }
        if (status !in 1..ARCHIVE_MAX_BYTES) {
            val message = when (status) {
                -312 -> "KAGEMUSHA durable testnet value owner is unavailable"
                -311 -> "KAGEMUSHA testnet value admission was rejected"
                else -> "KAGEMUSHA testnet value admission failed: $status"
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
        private const val ARCHIVE_MAX_BYTES = 768
        private val EXPECTED_CONTRACT = intArrayOf(1, OPERATION_ID_BYTES, ARCHIVE_MAX_BYTES)

        /** Open only the exact source-matched native contract. */
        @JvmStatic
        fun open(): KagemushaTestnetValueAdmissionV1 {
            try {
                System.loadLibrary("connect_norito_bridge")
                return openEndpoint(KagemushaTestnetValueAdmissionJniV1)
            } catch (error: LinkageError) {
                throw IllegalStateException("KAGEMUSHA testnet value JNI is unavailable", error)
            }
        }

        internal fun openEndpoint(
            endpoint: KagemushaTestnetValueAdmissionEndpointV1,
        ): KagemushaTestnetValueAdmissionV1 {
            val contract = try {
                endpoint.contract()
            } catch (error: LinkageError) {
                throw IllegalStateException("KAGEMUSHA testnet value JNI is unavailable", error)
            }
            check(contract?.contentEquals(EXPECTED_CONTRACT) == true) {
                "KAGEMUSHA testnet value JNI contract mismatch"
            }
            return KagemushaTestnetValueAdmissionV1(endpoint)
        }
    }
}

/** Rust is the only implementation; Kotlin supplies no synthetic verification path. */
internal object KagemushaTestnetValueAdmissionJniV1 : KagemushaTestnetValueAdmissionEndpointV1 {
    override fun contract(): IntArray? = nativeContractV1()

    override fun admit(operationId: ByteArray, output: ByteBuffer): Int =
        nativeAdmitV1(operationId, output)

    @JvmStatic private external fun nativeContractV1(): IntArray?
    @JvmStatic private external fun nativeAdmitV1(operationId: ByteArray, output: ByteBuffer): Int
}
