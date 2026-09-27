// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.probe

import java.nio.ByteBuffer

/** Exact JNI boundary for an already reserved, finalized testnet mint observation. */
internal interface KagemushaTestnetFinalizedMintObservationEndpointV1 {
    fun contract(): IntArray?
    fun observe(
        operationId: ByteArray,
        statusJson: ByteArray,
        anchorNetworkId: ByteArray,
        anchorHeightBits: Long,
        anchorContextId: ByteArray,
        statePublicInputs: ByteArray,
        pairedProof: ByteArray,
        output: ByteBuffer,
    ): Int
}

/**
 * Inspect the Applied top-up, independently pinned finality, and paired MintFold proof.
 *
 * Rust must install a release-authenticated durable observation owner, reserve the original
 * native-only mint opening before submission, and pin an independently authenticated chain
 * context for that operation. This transport cannot create either pin, qualify app or device
 * hardware, admit offline value, or issue a spendable credential.
 */
class KagemushaTestnetFinalizedMintObservationV1 private constructor(
    private val endpoint: KagemushaTestnetFinalizedMintObservationEndpointV1,
) {
    /**
     * Send the original bounded Torii status JSON, independently authenticated finality
     * coordinates, and canonical Norito proof inputs to Rust. The height is unsigned u64 bits;
     * all coordinates must match the previously authenticated native operation pin. A status
     * lookup hint is not an independent finality source.
     * Return only its explicitly unqualified observation archive.
     */
    fun observeFinalizedMint(
        operationId: ByteArray,
        statusJson: ByteArray,
        anchorNetworkId: ByteArray,
        anchorHeightBits: Long,
        anchorContextId: ByteArray,
        statePublicInputs: ByteArray,
        pairedProof: ByteArray,
    ): ByteArray {
        require(operationId.size == OPERATION_ID_BYTES && operationId.any { it != 0.toByte() }) {
            "KAGEMUSHA testnet mint operation ID is invalid"
        }
        require(statusJson.size in 1..STATUS_JSON_MAX_BYTES) {
            "KAGEMUSHA testnet mint status JSON size is invalid"
        }
        require(anchorNetworkId.size == ANCHOR_HASH_BYTES &&
            (anchorNetworkId.last().toInt() and 1) == 1 &&
            anchorContextId.size == ANCHOR_HASH_BYTES &&
            (anchorContextId.last().toInt() and 1) == 1 && anchorHeightBits != 0L) {
            "KAGEMUSHA testnet mint independent finality anchor is invalid"
        }
        require(statePublicInputs.size in 1..STATE_INPUT_MAX_BYTES) {
            "KAGEMUSHA testnet MintFold input size is invalid"
        }
        require(pairedProof.size in 1..PAIRED_PROOF_MAX_BYTES) {
            "KAGEMUSHA testnet MintFold proof size is invalid"
        }
        // All Java allocation and input copying precede the native trial-head advancement.
        val output = ByteBuffer.allocateDirect(OBSERVATION_MAX_BYTES)
        val status = try {
            endpoint.observe(operationId.copyOf(), statusJson.copyOf(), anchorNetworkId.copyOf(),
                anchorHeightBits, anchorContextId.copyOf(), statePublicInputs.copyOf(),
                pairedProof.copyOf(), output)
        } catch (error: LinkageError) {
            throw IllegalStateException("KAGEMUSHA testnet mint observer JNI is unavailable", error)
        }
        if (status !in 1..OBSERVATION_MAX_BYTES) {
            val message = when (status) {
                -312 -> "KAGEMUSHA durable testnet mint owner is unavailable"
                -311 -> "KAGEMUSHA testnet finalized mint was rejected"
                else -> "KAGEMUSHA testnet finalized mint observation failed: $status"
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
        private const val STATUS_JSON_MAX_BYTES = 16 * 1024 * 1024
        private const val ANCHOR_HASH_BYTES = 32
        private const val STATE_INPUT_MAX_BYTES = 4096
        private const val PAIRED_PROOF_MAX_BYTES = 6528
        private const val OBSERVATION_MAX_BYTES = 512
        private val EXPECTED_CONTRACT = intArrayOf(1, OPERATION_ID_BYTES, STATUS_JSON_MAX_BYTES,
            ANCHOR_HASH_BYTES, ANCHOR_HASH_BYTES, STATE_INPUT_MAX_BYTES, PAIRED_PROOF_MAX_BYTES,
            OBSERVATION_MAX_BYTES)

        /** Check the entire native contract without installing or authorizing an owner. */
        @JvmStatic
        fun open(): KagemushaTestnetFinalizedMintObservationV1 {
            try {
                System.loadLibrary("connect_norito_bridge")
                return openEndpoint(KagemushaTestnetFinalizedMintObservationJniV1)
            } catch (error: LinkageError) {
                throw IllegalStateException("KAGEMUSHA testnet mint observer JNI is unavailable", error)
            }
        }

        internal fun openEndpoint(
            endpoint: KagemushaTestnetFinalizedMintObservationEndpointV1,
        ): KagemushaTestnetFinalizedMintObservationV1 {
            val contract = try {
                endpoint.contract()
            } catch (error: LinkageError) {
                throw IllegalStateException("KAGEMUSHA testnet mint observer JNI is unavailable", error)
            }
            check(contract?.contentEquals(EXPECTED_CONTRACT) == true) {
                "KAGEMUSHA testnet mint observer contract mismatch"
            }
            return KagemushaTestnetFinalizedMintObservationV1(endpoint)
        }
    }
}

/** Rust is the sole implementation; there is no Kotlin verifier or synthetic success path. */
internal object KagemushaTestnetFinalizedMintObservationJniV1 :
    KagemushaTestnetFinalizedMintObservationEndpointV1 {
    override fun contract(): IntArray? = nativeContractV1()

    override fun observe(
        operationId: ByteArray,
        statusJson: ByteArray,
        anchorNetworkId: ByteArray,
        anchorHeightBits: Long,
        anchorContextId: ByteArray,
        statePublicInputs: ByteArray,
        pairedProof: ByteArray,
        output: ByteBuffer,
    ): Int = nativeObserveV1(operationId, statusJson, anchorNetworkId,
        anchorHeightBits, anchorContextId, statePublicInputs, pairedProof, output)

    @JvmStatic private external fun nativeContractV1(): IntArray?
    @JvmStatic private external fun nativeObserveV1(
        operationId: ByteArray,
        statusJson: ByteArray,
        anchorNetworkId: ByteArray,
        anchorHeightBits: Long,
        anchorContextId: ByteArray,
        statePublicInputs: ByteArray,
        pairedProof: ByteArray,
        output: ByteBuffer,
    ): Int
}
