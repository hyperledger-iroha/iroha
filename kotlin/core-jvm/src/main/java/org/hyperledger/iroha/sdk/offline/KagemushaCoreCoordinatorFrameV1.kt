// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import java.nio.ByteOrder

/** Closed native coordinator methods. Frame schema 2 is the sole supported V1 protocol frame. */
enum class KagemushaCoreCoordinatorMethodV1(@JvmField val code: Int) {
    RESERVE_OPERATION_ID(1), ACCEPT_QUALIFICATION(2), ACCEPT_AUTHENTICATED_REPLY(3),
    BEGIN_SENDER_TRANSITION(4), PROVE_PREPARED_SENDER_TRANSITION(5), BUILD_TERMINAL_ENVELOPE(6),
    ACCEPT_INSTALLED_TERMINAL(7), RECOVER_SENDER(8), RECOVER_TERMINAL_ENVELOPE(9), RELEASE_OUTBOX(10),
}

/**
 * Exact native ABI framing and request/response correlation, without granting monetary authority.
 *
 * Embedded Norito preparation/candidate/recovery archives remain opaque here. A qualified native
 * coordinator must authenticate their semantics; successful frame validation is not proof validation.
 */
object KagemushaCoreCoordinatorFrameV1 {
    const val SCHEMA_VERSION = 2
    const val MAXIMUM_FIELDS = 16
    const val MAXIMUM_FIELD_BYTES = 64 * 1024
    const val MAXIMUM_REQUEST_BYTES = 256 * 1024
    const val MAXIMUM_RESPONSE_BYTES = 128 * 1024
    private val magic = "IKGMCOR1".toByteArray(Charsets.US_ASCII)

    /** Encode only a complete method-specific request with the current native schema. */
    @JvmStatic
    fun encodeRequest(method: KagemushaCoreCoordinatorMethodV1, fields: List<ByteArray>): ByteArray {
        encodedSize(fields, MAXIMUM_REQUEST_BYTES)
        val retained = fields.map { it.copyOf() }
        validateRequestFields(method, retained)
        return encode(retained, MAXIMUM_REQUEST_BYTES)
    }

    /** Decode exact request bytes and reject retired schemas, tails, and invalid method fields. */
    @JvmStatic
    fun decodeRequest(method: KagemushaCoreCoordinatorMethodV1, frame: ByteArray): List<ByteArray> =
        decode(frame, MAXIMUM_REQUEST_BYTES).also { validateRequestFields(method, it) }

    /** Decode a response only after correlating its operation, terminal, and envelope to the request. */
    @JvmStatic
    fun decodeResponse(
        method: KagemushaCoreCoordinatorMethodV1,
        requestFrame: ByteArray,
        responseFrame: ByteArray,
    ): List<ByteArray> {
        val request = decodeRequest(method, requestFrame)
        return decode(responseFrame, MAXIMUM_RESPONSE_BYTES).also { validateResponseFields(method, request, it) }
    }

    /** Frame JNI's field-array response after applying the same strict correlation as the C ABI. */
    @JvmStatic
    fun encodeResponse(
        method: KagemushaCoreCoordinatorMethodV1,
        requestFrame: ByteArray,
        fields: List<ByteArray>,
    ): ByteArray {
        val request = decodeRequest(method, requestFrame)
        encodedSize(fields, MAXIMUM_RESPONSE_BYTES)
        val retained = fields.map { it.copyOf() }
        validateResponseFields(method, request, retained)
        return encode(retained, MAXIMUM_RESPONSE_BYTES)
    }

    /** Canonical little-endian native discriminant, suitable for method request fields. */
    @JvmStatic
    fun u32(value: Int): ByteArray = ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN).putInt(value).array()

    private fun encode(fields: List<ByteArray>, maximum: Int): ByteArray {
        val size = encodedSize(fields, maximum)
        val buffer = ByteBuffer.allocate(size).order(ByteOrder.LITTLE_ENDIAN)
        buffer.put(magic).putShort(SCHEMA_VERSION.toShort()).putShort(fields.size.toShort()).putInt(0)
        fields.forEach { buffer.putInt(it.size).put(it) }
        return buffer.array()
    }

    private fun encodedSize(fields: List<ByteArray>, maximum: Int): Int {
        require(fields.size <= MAXIMUM_FIELDS) { "too many coordinator fields" }
        var size = 16
        fields.forEach {
            require(it.size <= MAXIMUM_FIELD_BYTES) { "oversized coordinator field" }
            size += 4 + it.size
        }
        require(size <= maximum) { "oversized coordinator frame" }
        return size
    }

    private fun decode(frame: ByteArray, maximum: Int): List<ByteArray> {
        require(frame.size in 16..maximum) { "invalid coordinator frame size" }
        val buffer = ByteBuffer.wrap(frame.copyOf()).order(ByteOrder.LITTLE_ENDIAN)
        val prefix = ByteArray(8).also { buffer.get(it) }
        require(prefix.contentEquals(magic) && buffer.short.toInt() == SCHEMA_VERSION) { "invalid coordinator schema" }
        val count = buffer.short.toInt() and 0xffff
        require(count <= MAXIMUM_FIELDS && buffer.int == 0) { "invalid coordinator frame header" }
        val result = ArrayList<ByteArray>(count)
        repeat(count) {
            require(buffer.remaining() >= 4) { "truncated coordinator field length" }
            val length = buffer.int
            require(length in 0..MAXIMUM_FIELD_BYTES && length <= buffer.remaining()) { "invalid coordinator field length" }
            result.add(ByteArray(length).also { buffer.get(it) })
        }
        require(!buffer.hasRemaining()) { "trailing coordinator bytes" }
        return result.toList()
    }

    private fun validateRequestFields(method: KagemushaCoreCoordinatorMethodV1, fields: List<ByteArray>) {
        when (method) {
            KagemushaCoreCoordinatorMethodV1.RESERVE_OPERATION_ID -> {
                count(fields, 3); operation(fields, 0); digest(fields, 1); nonempty(fields, 2)
            }
            KagemushaCoreCoordinatorMethodV1.ACCEPT_QUALIFICATION -> {
                count(fields, 6); qualification(fields, 0); digest(fields, 5)
            }
            KagemushaCoreCoordinatorMethodV1.ACCEPT_AUTHENTICATED_REPLY -> {
                count(fields, 9); operation(fields, 0); digest(fields, 1)
                nonempty(fields, 2); nonempty(fields, 3); qualification(fields, 4)
            }
            KagemushaCoreCoordinatorMethodV1.BEGIN_SENDER_TRANSITION -> {
                digest(fields, 0)
                val end = senderInputs(fields, 1)
                count(fields, end + 5); qualification(fields, end)
            }
            KagemushaCoreCoordinatorMethodV1.PROVE_PREPARED_SENDER_TRANSITION,
            KagemushaCoreCoordinatorMethodV1.BUILD_TERMINAL_ENVELOPE,
            KagemushaCoreCoordinatorMethodV1.RECOVER_TERMINAL_ENVELOPE -> {
                count(fields, 2); nonempty(fields, 0); nonempty(fields, 1)
            }
            KagemushaCoreCoordinatorMethodV1.ACCEPT_INSTALLED_TERMINAL -> {
                count(fields, 5); fields.indices.forEach { nonempty(fields, it) }
            }
            KagemushaCoreCoordinatorMethodV1.RECOVER_SENDER -> {
                count(fields, 8)
                require(fields[0].contentEquals(byteArrayOf(0)) || fields[0].contentEquals(byteArrayOf(1))) { "invalid coordinator recovery selector" }
                digest(fields, 1); kind(fields, 2); qualification(fields, 3)
            }
            KagemushaCoreCoordinatorMethodV1.RELEASE_OUTBOX -> {
                digest(fields, 0)
                val end = senderInputs(fields, 1)
                count(fields, end + 7); nonempty(fields, end)
                val receipt = field(fields, end + 1)
                require(receipt.size > 4 && receipt.copyOfRange(0, 4).contentEquals(field(fields, 1))) { "invalid coordinator terminal receipt" }
                qualification(fields, end + 2)
            }
        }
    }

    private fun validateResponseFields(method: KagemushaCoreCoordinatorMethodV1, request: List<ByteArray>, response: List<ByteArray>) {
        when (method) {
            KagemushaCoreCoordinatorMethodV1.RESERVE_OPERATION_ID -> {
                count(response, 1); digest(response, 0); equal(response, 0, request, 1)
            }
            KagemushaCoreCoordinatorMethodV1.ACCEPT_QUALIFICATION,
            KagemushaCoreCoordinatorMethodV1.ACCEPT_AUTHENTICATED_REPLY -> count(response, 0)
            KagemushaCoreCoordinatorMethodV1.BEGIN_SENDER_TRANSITION -> {
                count(response, 2); digest(response, 0); nonempty(response, 1); equal(response, 0, request, 0)
            }
            KagemushaCoreCoordinatorMethodV1.PROVE_PREPARED_SENDER_TRANSITION,
            KagemushaCoreCoordinatorMethodV1.BUILD_TERMINAL_ENVELOPE,
            KagemushaCoreCoordinatorMethodV1.RECOVER_TERMINAL_ENVELOPE -> {
                count(response, 1); nonempty(response, 0)
            }
            KagemushaCoreCoordinatorMethodV1.ACCEPT_INSTALLED_TERMINAL -> {
                count(response, 2); nonempty(response, 0); nonempty(response, 1); equal(response, 0, request, 1)
            }
            KagemushaCoreCoordinatorMethodV1.RECOVER_SENDER -> {
                if (response.isEmpty()) return
                count(response, 3); digest(response, 0); digest(response, 1); nonempty(response, 2)
                equal(response, if (request[0][0].toInt() == 0) 1 else 0, request, 1)
            }
            KagemushaCoreCoordinatorMethodV1.RELEASE_OUTBOX -> {
                count(response, 5); digest(response, 0); nonempty(response, 1); digest(response, 2)
                nonempty(response, 3); nonempty(response, 4); equal(response, 3, request, senderInputs(request, 1))
            }
        }
    }

    private fun field(fields: List<ByteArray>, index: Int): ByteArray =
        requireNotNull(fields.getOrNull(index)) { "missing coordinator field" }

    private fun count(fields: List<ByteArray>, expected: Int) {
        require(fields.size == expected) { "invalid coordinator field count" }
    }

    private fun nonempty(fields: List<ByteArray>, index: Int) {
        require(field(fields, index).isNotEmpty()) { "empty coordinator field" }
    }

    private fun digest(fields: List<ByteArray>, index: Int) {
        val bytes = field(fields, index)
        require(bytes.size == 32 && bytes.any { it.toInt() != 0 }) { "invalid coordinator digest" }
    }

    private fun number(fields: List<ByteArray>, index: Int): Int {
        val bytes = field(fields, index)
        require(bytes.size == 4) { "invalid coordinator u32" }
        return ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN).int
    }

    private fun operation(fields: List<ByteArray>, index: Int) {
        require(number(fields, index) in 1..22) { "invalid coordinator device operation" }
    }

    private fun kind(fields: List<ByteArray>, index: Int): Int = number(fields, index).also {
        require(it in 0..1) { "invalid coordinator sender kind" }
    }

    private fun senderInputs(fields: List<ByteArray>, start: Int): Int =
        if (kind(fields, start) == 0) {
            nonempty(fields, start + 1); start + 2
        } else {
            val amount = field(fields, start + 1)
            require(amount.size == 16 && amount.any { it.toInt() != 0 }) { "invalid coordinator positive u128" }
            nonempty(fields, start + 2); start + 3
        }

    private fun qualification(fields: List<ByteArray>, start: Int) {
        require(number(fields, start) == 1) { "invalid coordinator protocol version" }
        digest(fields, start + 1); nonempty(fields, start + 2); nonempty(fields, start + 3)
        require(number(fields, start + 4) == 0xffff) { "incomplete coordinator capabilities" }
    }

    private fun equal(left: List<ByteArray>, li: Int, right: List<ByteArray>, ri: Int) {
        require(field(left, li).contentEquals(field(right, ri))) { "coordinator response substituted request binding" }
    }
}
