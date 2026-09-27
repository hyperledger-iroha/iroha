// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.probe

import java.nio.ByteBuffer
import java.nio.ByteOrder
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash

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
        val requestedOperationId = operationId.copyOf()
        val output = ByteBuffer.allocateDirect(ARCHIVE_MAX_BYTES)
        val status = try {
            endpoint.admit(requestedOperationId.copyOf(), output)
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
        requireCanonicalAdmissionArchive(archive, requestedOperationId)
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

private const val ADMISSION_ARCHIVE_SCHEMA_V1 =
    "connect_norito_bridge::KagemushaTestnetValueAdmissionArchiveV1"
private const val ADMISSION_PAYLOAD_BYTES_V1 = 432
private const val ADMISSION_ALIGNMENT_BYTES_V1 = 16
private const val ADMISSION_PADDING_BYTES_V1 =
    (ADMISSION_ALIGNMENT_BYTES_V1 - NoritoHeader.HEADER_LENGTH % ADMISSION_ALIGNMENT_BYTES_V1) %
        ADMISSION_ALIGNMENT_BYTES_V1
private const val ADMISSION_FRAME_BYTES_V1 =
    NoritoHeader.HEADER_LENGTH + ADMISSION_PADDING_BYTES_V1 + ADMISSION_PAYLOAD_BYTES_V1

private fun requireCanonicalAdmissionArchive(archive: ByteArray, requestedOperationId: ByteArray) {
    require(archive.size == ADMISSION_FRAME_BYTES_V1) {
        "KAGEMUSHA testnet value admission archive length is invalid"
    }
    require(archive.copyOfRange(NoritoHeader.HEADER_LENGTH,
        NoritoHeader.HEADER_LENGTH + ADMISSION_PADDING_BYTES_V1).all { it == 0.toByte() }) {
        "KAGEMUSHA testnet value admission archive padding is not canonical"
    }
    val frame = NoritoHeader.decode(archive, SchemaHash.hash16(ADMISSION_ARCHIVE_SCHEMA_V1))
    require(frame.header.compression == NoritoHeader.COMPRESSION_NONE &&
        frame.header.flags == NoritoHeader.COMPACT_LEN &&
        frame.header.payloadLength == ADMISSION_PAYLOAD_BYTES_V1 &&
        frame.header.encode().contentEquals(archive.copyOfRange(0, NoritoHeader.HEADER_LENGTH))) {
        "KAGEMUSHA testnet value admission archive framing is not canonical"
    }
    frame.header.validateChecksum(frame.payload)
    val decoder = NoritoDecoder(frame.payload, NoritoHeader.COMPACT_LEN)
    val version = ByteBuffer.wrap(decoder.readAdmissionField(2)).order(ByteOrder.LITTLE_ENDIAN)
        .short.toInt() and 0xffff
    val hardwareQualified = decoder.readAdmissionField(1)[0].toInt()
    val networkId = decoder.readAdmissionField(32)
    val releaseId = decoder.readAdmissionField(32)
    val releaseAttestationDigest = decoder.readAdmissionField(32)
    val assetIdentityDigest = decoder.readAdmissionField(32)
    val assetIncarnation = decoder.readAdmissionField(32)
    val assetScale = ByteBuffer.wrap(decoder.readAdmissionField(4)).order(ByteOrder.LITTLE_ENDIAN)
        .int.toLong() and 0xffff_ffffL
    val liabilityPoolId = decoder.readAdmissionField(32)
    val operationId = decoder.readAdmissionField(32)
    val creditId = decoder.readAdmissionField(32)
    val amount = decoder.readAdmissionField(16)
    val mintEnvelopeDigest = decoder.readAdmissionField(32)
    val candidateEnvelopeDigest = decoder.readAdmissionField(32)
    val successorStateCommitment = decoder.readAdmissionField(32)
    val finalityHeightBits = ByteBuffer.wrap(decoder.readAdmissionField(8))
        .order(ByteOrder.LITTLE_ENDIAN).long
    val finalityHeightContextId = decoder.readAdmissionField(32)
    require(decoder.remaining() == 0 && version == 1 && hardwareQualified == 0 &&
        assetScale <= 28L && finalityHeightBits != 0L &&
        amount.any { it != 0.toByte() } && operationId.contentEquals(requestedOperationId) &&
        listOf(networkId, releaseId, releaseAttestationDigest, assetIdentityDigest,
            assetIncarnation, liabilityPoolId, creditId, mintEnvelopeDigest,
            candidateEnvelopeDigest, successorStateCommitment, finalityHeightContextId)
            .all { digest -> digest.any { it != 0.toByte() } } &&
        !networkId.contentEquals(releaseId) &&
        !networkId.contentEquals(releaseAttestationDigest) &&
        !releaseId.contentEquals(releaseAttestationDigest) &&
        !assetIdentityDigest.contentEquals(liabilityPoolId)) {
        "KAGEMUSHA testnet value admission archive facts are invalid"
    }
}

private fun NoritoDecoder.readAdmissionField(length: Int): ByteArray {
    // Fixed-width fields below 128 bytes have a one-byte canonical compact length.
    require(readByte() == length) { "KAGEMUSHA testnet value admission field length is not canonical" }
    return readBytes(length)
}

/** Rust is the only implementation; Kotlin supplies no synthetic verification path. */
internal object KagemushaTestnetValueAdmissionJniV1 : KagemushaTestnetValueAdmissionEndpointV1 {
    override fun contract(): IntArray? = nativeContractV1()

    override fun admit(operationId: ByteArray, output: ByteBuffer): Int =
        nativeAdmitV1(operationId, output)

    @JvmStatic private external fun nativeContractV1(): IntArray?
    @JvmStatic private external fun nativeAdmitV1(operationId: ByteArray, output: ByteBuffer): Int
}
