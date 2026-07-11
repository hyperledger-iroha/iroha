@file:OptIn(ExperimentalEncodingApi::class)

package org.hyperledger.iroha.sdk.core.model.instructions

import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi

internal object ReplicationOrderInstructionValidation {
    private const val ORDER_ID_BYTES = 32
    private const val MAX_ORDER_PAYLOAD_BYTES = 1024 * 1024
    private val canonicalOrderIdPattern = Regex("^[0-9a-f]{64}$")
    private val hexAlphabet = "0123456789abcdef".toCharArray()

    fun requireOrderId(value: String): String {
        require(canonicalOrderIdPattern.matches(value)) {
            "orderIdHex must contain exactly 64 lowercase hexadecimal characters"
        }
        require(value.any { it != '0' }) { "orderIdHex must not be the zero identifier" }
        return value
    }

    fun encodeOrderId(value: ByteArray): String {
        require(value.size == ORDER_ID_BYTES) {
            "orderId must contain exactly $ORDER_ID_BYTES bytes, found ${value.size}"
        }
        require(value.any { it.toInt() != 0 }) { "orderId must not be the zero identifier" }
        val encoded = CharArray(value.size * 2)
        value.forEachIndexed { index, byte ->
            val unsigned = byte.toInt() and 0xff
            encoded[index * 2] = hexAlphabet[unsigned ushr 4]
            encoded[index * 2 + 1] = hexAlphabet[unsigned and 0x0f]
        }
        return encoded.concatToString()
    }

    fun requireCanonicalPayload(value: String): String {
        require(value.isNotEmpty()) { "orderPayloadBase64 must not be empty" }
        val decoded = try {
            Base64.decode(value)
        } catch (ex: IllegalArgumentException) {
            throw IllegalArgumentException("orderPayloadBase64 must be canonical base64", ex)
        }
        require(decoded.isNotEmpty()) { "orderPayloadBase64 must decode to non-empty bytes" }
        require(decoded.size <= MAX_ORDER_PAYLOAD_BYTES) {
            "orderPayloadBase64 decodes to ${decoded.size} bytes; maximum is $MAX_ORDER_PAYLOAD_BYTES"
        }
        require(Base64.encode(decoded) == value) { "orderPayloadBase64 must use canonical base64" }
        return value
    }

    fun requireEpoch(value: Long, fieldName: String): Long {
        require(value >= 0) { "$fieldName must be non-negative" }
        return value
    }

    fun requireWindow(issuedEpoch: Long, deadlineEpoch: Long) {
        require(deadlineEpoch > issuedEpoch) { "deadlineEpoch must be greater than issuedEpoch" }
    }

    fun requireArguments(
        arguments: Map<String, String>,
        action: String,
        fields: Set<String>,
    ) {
        val expected = fields + "action"
        require(arguments.keys == expected) {
            "Instruction arguments must contain exactly ${expected.sorted()}"
        }
        require(arguments["action"] == action) {
            "Instruction argument 'action' must be '$action'"
        }
    }
}
