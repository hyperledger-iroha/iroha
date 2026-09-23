// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.probe

import java.nio.ByteBuffer

/** Narrow JNI endpoint for one non-authorizing, process-owned testnet proof trial. */
internal interface KagemushaTestnetStateProofObservationEndpointV1 {
    fun contract(): IntArray?
    fun observe(publicInputsArchive: ByteArray, pairedProofArchive: ByteArray, output: ByteBuffer): Int
}

/** A diagnostic rejection; [status] is the native code or an invalid reported archive length. */
class KagemushaTestnetObservationExceptionV1(
    val status: Int,
    message: String,
) : IllegalStateException(message)

/**
 * Observe an actual paired State proof against a Rust-installed, operator-pinned testnet release.
 *
 * The result is an unsigned canonical Norito diagnostic archive. It is not a payment, admission,
 * hardware qualification, or monetary-authority credential. A stock app has no installed verifier
 * owner; its calls fail with status -312 until a signed release is independently installed by Rust.
 */
class KagemushaTestnetStateProofObservationV1 private constructor(
    private val endpoint: KagemushaTestnetStateProofObservationEndpointV1,
) {
    /** Verify one bounded canonical State statement and paired proof, then return its diagnostic archive. */
    fun observeStateProof(publicInputsArchive: ByteArray, pairedProofArchive: ByteArray): ByteArray {
        require(publicInputsArchive.size in 1..STATE_INPUT_MAX_BYTES) {
            "KAGEMUSHA testnet State input size is invalid"
        }
        require(pairedProofArchive.size in 1..PAIRED_PROOF_MAX_BYTES) {
            "KAGEMUSHA testnet paired proof size is invalid"
        }
        // Direct memory is allocated before the native verifier can advance its trial head.
        // The JNI entry writes the complete response into this same buffer before success.
        val output = ByteBuffer.allocateDirect(OBSERVATION_MAX_BYTES)
        val status = try {
            endpoint.observe(publicInputsArchive, pairedProofArchive, output)
        } catch (error: LinkageError) {
            throw IllegalStateException("KAGEMUSHA testnet proof observer JNI is unavailable", error)
        }
        if (status !in 1..OBSERVATION_MAX_BYTES) {
            val message = when (status) {
                -312 -> "KAGEMUSHA testnet proof verifier owner is unavailable"
                -311 -> "KAGEMUSHA testnet State proof was rejected"
                else -> "KAGEMUSHA testnet proof observation failed: $status"
            }
            throw KagemushaTestnetObservationExceptionV1(status, message)
        }
        val archive = ByteArray(status)
        output.position(0)
        output.get(archive)
        return archive
    }

    companion object {
        private const val STATE_INPUT_MAX_BYTES = 4096
        private const val PAIRED_PROOF_MAX_BYTES = 6528
        private const val OBSERVATION_MAX_BYTES = 256
        private val EXPECTED_CONTRACT = intArrayOf(1, STATE_INPUT_MAX_BYTES, PAIRED_PROOF_MAX_BYTES, OBSERVATION_MAX_BYTES)

        /** Load and check the exact diagnostic JNI ABI; this does not install a verifier owner. */
        @JvmStatic
        fun open(): KagemushaTestnetStateProofObservationV1 {
            try {
                System.loadLibrary("connect_norito_bridge")
                return openEndpoint(KagemushaTestnetStateProofObservationJniV1)
            } catch (error: LinkageError) {
                throw IllegalStateException("KAGEMUSHA testnet proof observer JNI is unavailable", error)
            }
        }

        internal fun openEndpoint(
            endpoint: KagemushaTestnetStateProofObservationEndpointV1,
        ): KagemushaTestnetStateProofObservationV1 {
            val contract = try {
                endpoint.contract()
            } catch (error: LinkageError) {
                throw IllegalStateException("KAGEMUSHA testnet proof observer JNI is unavailable", error)
            }
            check(contract?.contentEquals(EXPECTED_CONTRACT) == true) {
                "KAGEMUSHA testnet proof observer contract mismatch"
            }
            return KagemushaTestnetStateProofObservationV1(endpoint)
        }
    }
}

/** JNI symbols are owned by the Rust diagnostic observer and have no Kotlin verifier fallback. */
internal object KagemushaTestnetStateProofObservationJniV1 : KagemushaTestnetStateProofObservationEndpointV1 {
    override fun contract(): IntArray? = nativeContractV1()

    override fun observe(
        publicInputsArchive: ByteArray,
        pairedProofArchive: ByteArray,
        output: ByteBuffer,
    ): Int = nativeObserveV1(publicInputsArchive, pairedProofArchive, output)

    @JvmStatic private external fun nativeContractV1(): IntArray?
    @JvmStatic private external fun nativeObserveV1(
        publicInputsArchive: ByteArray,
        pairedProofArchive: ByteArray,
        output: ByteBuffer,
    ): Int
}
