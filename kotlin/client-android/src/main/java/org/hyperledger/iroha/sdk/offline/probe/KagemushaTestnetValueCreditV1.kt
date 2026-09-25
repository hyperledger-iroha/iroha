// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.probe

import java.math.BigDecimal
import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.ByteOrder
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash

/** Copyable release, network, and asset labels reported by one native testnet credit. */
data class KagemushaTestnetMintCreditScopeV1(
    val networkIdHex: String,
    val releaseIdHex: String,
    val releaseAttestationDigestHex: String,
    val assetIdentityDigestHex: String,
    val assetIncarnationHex: String,
    val assetScale: Int,
    val liabilityPoolIdHex: String,
)

/** Copyable inspection of one durable native testnet credit, never a payment credential. */
data class KagemushaTestnetMintLedgerCreditV1(
    val scope: KagemushaTestnetMintCreditScopeV1,
    val operationIdHex: String,
    val creditIdHex: String,
    val amountAtomic: BigInteger,
    val totalAdmittedAtomic: BigInteger,
) {
    /** Exact decimal credit at the asset's signed scale. */
    fun amountDecimal(): BigDecimal = BigDecimal(amountAtomic, scope.assetScale)

    /** Exact decimal total of credits counted by this native ledger. */
    fun totalAdmittedDecimal(): BigDecimal = BigDecimal(totalAdmittedAtomic, scope.assetScale)

    /** These copyable fields never assert production hardware or spending authority. */
    val testnetOnly: Boolean get() = true
    val hardwareQualified: Boolean get() = false
    val productionMonetaryAuthorized: Boolean get() = false
}

/** Exact JNI transport for one credit already committed by the native testnet ledger. */
internal interface KagemushaTestnetValueCreditEndpointV1 {
    fun contract(): IntArray?
    fun credit(operationId: ByteArray, output: ByteBuffer): Int
}

/** Inspect one Experimental mint credit by operation ID after durable native admission. */
class KagemushaTestnetValueCreditV1 private constructor(
    private val endpoint: KagemushaTestnetValueCreditEndpointV1,
) {
    /** Return exact counted testnet value and scope; this grants no spend authority. */
    fun creditFinalizedValue(operationId: ByteArray): KagemushaTestnetMintLedgerCreditV1 {
        require(operationId.size == OPERATION_ID_BYTES) {
            "KAGEMUSHA testnet credit operation ID is invalid"
        }
        val requestedOperationId = operationId.copyOf()
        require(requestedOperationId.any { it != 0.toByte() }) {
            "KAGEMUSHA testnet credit operation ID is invalid"
        }
        val output = ByteBuffer.allocateDirect(ARCHIVE_MAX_BYTES)
        val status = try {
            endpoint.credit(requestedOperationId.copyOf(), output)
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
        return decodeCreditArchive(archive, requestedOperationId)
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

private const val CREDIT_ARCHIVE_SCHEMA_V1 =
    "connect_norito_bridge::KagemushaTestnetMintLedgerCreditArchiveV1"
private const val CREDIT_PAYLOAD_BYTES_V1 = 308
private const val CREDIT_PADDING_BYTES_V1 = 9 // Native struct alignment is 16; header is 39 bytes.
private const val CREDIT_FRAME_BYTES_V1 =
    NoritoHeader.HEADER_LENGTH + CREDIT_PADDING_BYTES_V1 + CREDIT_PAYLOAD_BYTES_V1

private fun decodeCreditArchive(
    archive: ByteArray,
    requestedOperationId: ByteArray,
): KagemushaTestnetMintLedgerCreditV1 {
    require(archive.size == CREDIT_FRAME_BYTES_V1) { "KAGEMUSHA testnet credit archive length is invalid" }
    val frame = NoritoHeader.decode(archive, SchemaHash.hash16(CREDIT_ARCHIVE_SCHEMA_V1))
    require(frame.header.compression == NoritoHeader.COMPRESSION_NONE &&
        frame.header.flags == NoritoHeader.COMPACT_LEN &&
        frame.header.payloadLength == CREDIT_PAYLOAD_BYTES_V1 &&
        frame.header.encode().contentEquals(archive.copyOfRange(0, NoritoHeader.HEADER_LENGTH))) {
        "KAGEMUSHA testnet credit archive framing is not canonical"
    }
    frame.header.validateChecksum(frame.payload)
    val decoder = NoritoDecoder(frame.payload, NoritoHeader.COMPACT_LEN)
    val version = ByteBuffer.wrap(decoder.readCreditField(2)).order(ByteOrder.LITTLE_ENDIAN)
        .short.toInt() and 0xffff
    require(version == 1) { "KAGEMUSHA testnet credit archive version is invalid" }
    // The native Experimental ledger always writes false; true would mislabel this as qualified.
    require(decoder.readCreditField(1)[0].toInt() == 0) {
        "KAGEMUSHA testnet credit archive claims hardware qualification"
    }
    val networkId = decoder.readCreditField(32)
    val releaseId = decoder.readCreditField(32)
    val releaseAttestationDigest = decoder.readCreditField(32)
    val assetIdentityDigest = decoder.readCreditField(32)
    val assetIncarnation = decoder.readCreditField(32)
    val assetScale = ByteBuffer.wrap(decoder.readCreditField(4)).order(ByteOrder.LITTLE_ENDIAN)
        .int.toLong() and 0xffff_ffffL
    val liabilityPoolId = decoder.readCreditField(32)
    val operationId = decoder.readCreditField(32)
    val creditId = decoder.readCreditField(32)
    val amount = BigInteger(1, decoder.readCreditField(16).reversedArray())
    val totalAdmitted = BigInteger(1, decoder.readCreditField(16).reversedArray())
    require(decoder.remaining() == 0 && operationId.contentEquals(requestedOperationId) &&
        networkId.any { it != 0.toByte() } &&
        releaseId.any { it != 0.toByte() } &&
        releaseAttestationDigest.any { it != 0.toByte() } &&
        assetIdentityDigest.any { it != 0.toByte() } &&
        assetIncarnation.any { it != 0.toByte() } &&
        liabilityPoolId.any { it != 0.toByte() } &&
        creditId.any { it != 0.toByte() } &&
        !networkId.contentEquals(releaseId) &&
        !networkId.contentEquals(releaseAttestationDigest) &&
        !releaseId.contentEquals(releaseAttestationDigest) &&
        !assetIdentityDigest.contentEquals(liabilityPoolId) &&
        assetScale <= 28L && amount.signum() > 0 && totalAdmitted >= amount) {
        "KAGEMUSHA testnet credit archive facts are invalid"
    }
    return KagemushaTestnetMintLedgerCreditV1(
        KagemushaTestnetMintCreditScopeV1(
            networkId.toCreditHex(), releaseId.toCreditHex(),
            releaseAttestationDigest.toCreditHex(), assetIdentityDigest.toCreditHex(),
            assetIncarnation.toCreditHex(), assetScale.toInt(), liabilityPoolId.toCreditHex(),
        ),
        operationId.toCreditHex(), creditId.toCreditHex(), amount, totalAdmitted,
    )
}

private fun NoritoDecoder.readCreditField(length: Int): ByteArray {
    // Every field is fixed-width and below 128 bytes, so its sole canonical compact length
    // is one byte. This also rejects overlong varints that a general-purpose reader may accept.
    require(readByte() == length) { "KAGEMUSHA testnet credit field length is not canonical" }
    return readBytes(length)
}

private val CREDIT_HEX_DIGITS_V1 = "0123456789abcdef".toCharArray()

private fun ByteArray.toCreditHex(): String = buildString(size * 2) {
    for (byte in this@toCreditHex) {
        val value = byte.toInt() and 0xff
        append(CREDIT_HEX_DIGITS_V1[value ushr 4])
        append(CREDIT_HEX_DIGITS_V1[value and 0x0f])
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
