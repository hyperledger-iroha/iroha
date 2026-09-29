package org.hyperledger.iroha.sdk.offline.wallet

import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.client.UnverifiedKagemushaOperationStatusV1
import org.hyperledger.iroha.sdk.offline.*

/** Independently selected full checkpoint. Construction checks transport bounds only;
 * native verification checks canonical encoding, network and signatures before any result. */
class KagemushaFinalityTrustAnchorV1(networkId: ByteArray, checkpoint: ByteArray) {
    private val network = reserveFinalityHash(networkId)
    private val nativeCheckpoint: ByteArray
    init {
        require(checkpoint.size in 1..MAXIMUM_CHECKPOINT_BYTES)
        nativeCheckpoint = checkpoint.copyOf()
    }
    fun networkId(): ByteArray = network.copyOf()
    fun checkpoint(): ByteArray = nativeCheckpoint.copyOf()
    companion object { const val MAXIMUM_CHECKPOINT_BYTES = 68 * 1024 * 1024 }
}

/** Native-decoded routing metadata only. This type has no conversion into a trusted anchor. */
class KagemushaUntrustedFinalityHintV1 internal constructor(networkId: ByteArray, val blockHeight: BigInteger, blockHash: ByteArray) {
    private val network = reserveFinalityHash(networkId)
    private val block = reserveFinalityHash(blockHash)
    init { requireReserveFinalityHeight(blockHeight) }
    fun networkId(): ByteArray = network.copyOf()
    fun blockHash(): ByteArray = block.copyOf()
}

/**
 * Concrete native certificate/reserve-witness verification for an exact saved request.
 * A successful wire payload still requires qualified local Core admission before monetary use.
 * Keep original status evidence and independently authenticated anchor provenance for durable retry.
 */
object KagemushaReserveFinalityV1 {
    private const val MAXIMUM_STATUS_JSON_BYTES = 4 * (36 * 1024 * 1024 + 256)
    private const val REQUIRED_BRIDGE_ABI_VERSION = 25
    private val available: Boolean by lazy {
        try {
            System.loadLibrary("connect_norito_bridge")
            KagemushaReserveFinalityJniV1.nativeBridgeAbiVersion() == REQUIRED_BRIDGE_ABI_VERSION &&
                KagemushaReserveFinalityJniV1.nativeHint(byteArrayOf()) == null &&
                KagemushaReserveFinalityJniV1.nativeVerify(byteArrayOf(), 255, byteArrayOf(), byteArrayOf(), byteArrayOf()) == null
        } catch (_: LinkageError) { false } catch (_: RuntimeException) { false }
    }

    /** Requires every new JNI export as well as the sole admitted bridge ABI; no fallback. */
    @JvmStatic fun isAvailable(): Boolean = available

    /** Decodes only untrusted coordinates; neither callback nor result can release money. */
    @JvmStatic fun hint(status: UnverifiedKagemushaOperationStatusV1): KagemushaUntrustedFinalityHintV1? =
        status.verifyAgainst(Unit) { json, _ ->
            val response = requireResponse(json)
            requireAvailable()
            val projection = nativeResult { KagemushaReserveFinalityJniV1.nativeHint(response) }
            parseReserveFinalityHint(projection)
        }

    /** Authenticate the original full top-up request and release its exact device-bound credit. */
    @JvmStatic fun verifyTopUp(status: UnverifiedKagemushaOperationStatusV1,
        request: KagemushaTopUpRequestV1, anchor: KagemushaFinalityTrustAnchorV1): KagemushaMintCreditV1 =
        status.verifyAgainst(anchor) { json, trusted -> verifyTopUpJson(json, request, trusted) }

    /** Authenticate the original full redemption request and release its exact finalized voucher. */
    @JvmStatic fun verifyRedemption(status: UnverifiedKagemushaOperationStatusV1,
        request: KagemushaRedemptionRequestV1, anchor: KagemushaFinalityTrustAnchorV1): KagemushaRedemptionVoucherV1 =
        status.verifyAgainst(anchor) { json, trusted -> verifyRedemptionJson(json, request, trusted) }

    // Fixed native callbacks also serve Java clients of the Kotlin-owned status wrapper.
    @JvmStatic fun verifyTopUpJson(responseJson: ByteArray, request: KagemushaTopUpRequestV1,
        anchor: KagemushaFinalityTrustAnchorV1): KagemushaMintCreditV1 {
        val canonical = KagemushaNoritoV1.encodeTopUpRequestShape(request)
        val payload = verify(responseJson, 0, canonical, anchor, KagemushaWireV1.MAXIMUM_MINT_CREDIT_BYTES)
        return KagemushaNoritoV1.decodeMintCreditShapeExact(payload, checkNotNull(request.mintAuthorization))
    }

    @JvmStatic fun verifyRedemptionJson(responseJson: ByteArray, request: KagemushaRedemptionRequestV1,
        anchor: KagemushaFinalityTrustAnchorV1): KagemushaRedemptionVoucherV1 {
        val canonical = KagemushaNoritoV1.encodeRedemptionRequestShape(request)
        val payload = verify(responseJson, 1, canonical, anchor, KagemushaWireV1.MAXIMUM_REDEMPTION_VOUCHER_BYTES)
        check(payload.contentEquals(KagemushaNoritoV1.encodeRedemptionVoucherShape(request.voucher)))
        return KagemushaNoritoV1.decodeRedemptionVoucherShapeExact(payload)
    }

    private fun verify(json: ByteArray, kind: Int, request: ByteArray,
        anchor: KagemushaFinalityTrustAnchorV1, maximum: Int): ByteArray {
        val response = requireResponse(json)
        requireAvailable()
        val output = nativeResult { KagemushaReserveFinalityJniV1.nativeVerify(response, kind, request.copyOf(),
            anchor.networkId(), anchor.checkpoint()) }
        check(output.isNotEmpty() && output.size <= maximum) { "Native Offline finality output is out of bounds" }
        return output
    }

    private fun requireResponse(value: ByteArray): ByteArray {
        require(value.isNotEmpty() && value.size <= MAXIMUM_STATUS_JSON_BYTES)
        return value.copyOf()
    }
    private fun requireAvailable() { check(available) { "Native Offline finality verifier is unavailable" } }
    private inline fun nativeResult(invoke: () -> ByteArray?): ByteArray = try {
        checkNotNull(invoke()) { "Native Offline finality verification rejected" }
    } catch (_: LinkageError) { throw IllegalStateException("Native Offline finality verifier is unavailable") }
}

internal object KagemushaReserveFinalityJniV1 {
    @JvmStatic external fun nativeBridgeAbiVersion(): Int
    @JvmStatic external fun nativeHint(responseJson: ByteArray): ByteArray?
    @JvmStatic external fun nativeVerify(responseJson: ByteArray, kind: Int, expectedRequest: ByteArray,
        networkId: ByteArray, checkpoint: ByteArray): ByteArray?
}

internal fun reserveFinalityHash(value: ByteArray): ByteArray {
    require(value.size == 32 && (value.last().toInt() and 1) == 1)
    return value.copyOf()
}
internal fun requireReserveFinalityHeight(value: BigInteger) {
    require(value.signum() > 0 && value.bitLength() <= 64)
}
internal fun parseReserveFinalityHint(bytes: ByteArray): KagemushaUntrustedFinalityHintV1? {
    require(bytes.isNotEmpty() && bytes.size <= 512)
    val text = Charsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT)
        .onUnmappableCharacter(CodingErrorAction.REPORT).decode(ByteBuffer.wrap(bytes)).toString()
    val parsed = JsonParser.parse(text) ?: return null
    val value = parsed as? Map<*, *> ?: error("Invalid native finality hint")
    check(value.keys == setOf("version", "network_id", "block_height", "block_hash"))
    check(value["version"] is Number && value["version"].toString() == "1")
    fun hash(key: String): ByteArray {
        val encoded = value[key] as? String ?: error("Invalid native finality hint hash")
        require(encoded.length == 64 && encoded.all { it in '0'..'9' || it in 'a'..'f' })
        return reserveFinalityHash(encoded.chunked(2).map { it.toInt(16).toByte() }.toByteArray())
    }
    val height = value["block_height"] as? String ?: error("Invalid native finality hint height")
    require(height.length in 1..20 && height.all { it in '0'..'9' })
    val number = BigInteger(height)
    require(number.toString() == height)
    return KagemushaUntrustedFinalityHintV1(hash("network_id"), number, hash("block_hash"))
}
