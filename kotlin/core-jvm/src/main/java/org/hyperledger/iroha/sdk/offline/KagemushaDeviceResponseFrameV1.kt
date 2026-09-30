// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest

/** Exact original-device framing checks. These checks confer no signature or monetary authority. */
object KagemushaDeviceResponseFrameV1 {
    const val MAXIMUM_BYTES = 116 + 64 * 1024 + 64
    private val magic = "IKGMJRS1".toByteArray(Charsets.US_ASCII)

    /** Require a complete original success frame correlated with the selected operation and ID. */
    @JvmStatic
    fun requireSuccessShape(frame: ByteArray, operation: Int, requestId: ByteArray): ByteArray {
        require(requestId.size == 32 && requestId.any { it != 0.toByte() })
        val decoded = decode(frame)
        require(decoded.operation == operation && decoded.status == 0 &&
            decoded.requestId.contentEquals(requestId)) { "original device response identity mismatch" }
        return frame.copyOf()
    }

    internal fun requireTuple(frame: ByteArray, operation: Int, status: KagemushaAuthenticatedDeviceStatusV1,
        payload: ByteArray, authenticator: ByteArray, requestId: ByteArray? = null) {
        val decoded = decode(frame)
        require(decoded.operation == operation && decoded.status == status.code &&
            decoded.payload.contentEquals(payload) && decoded.authenticator.contentEquals(authenticator) &&
            (requestId == null || decoded.requestId.contentEquals(requestId))) {
            "original device response substituted its authenticated tuple"
        }
    }

    private class Fields(val operation: Int, val status: Int, val requestId: ByteArray,
        val payload: ByteArray, val authenticator: ByteArray)

    private fun decode(frame: ByteArray): Fields {
        require(frame.size in 116..MAXIMUM_BYTES) { "invalid original device response size" }
        val input = ByteBuffer.wrap(frame.copyOf()).order(ByteOrder.LITTLE_ENDIAN)
        require(ByteArray(8).also(input::get).contentEquals(magic) && input.short.toInt() == 1) {
            "invalid original device response schema"
        }
        val operation = input.get().toInt() and 0xff
        val status = input.get().toInt() and 0xff
        require(operation in 1..22 && status in 0..10)
        val id = ByteArray(32).also(input::get)
        require(id.any { it != 0.toByte() })
        val payloadSize = input.int
        val authenticatorSize = input.int
        require(payloadSize in 0..64 * 1024 && authenticatorSize in 0..64 &&
            input.remaining() == 64 + payloadSize + authenticatorSize) { "invalid original device response lengths" }
        val payloadHash = ByteArray(32).also(input::get)
        val authenticatorHash = ByteArray(32).also(input::get)
        val payload = ByteArray(payloadSize).also(input::get)
        val authenticator = ByteArray(authenticatorSize).also(input::get)
        require(MessageDigest.isEqual(payloadHash, sha256(payload)) &&
            MessageDigest.isEqual(authenticatorHash, sha256(authenticator))) { "original device response checksum mismatch" }
        if (status == 0) {
            require(payload.isNotEmpty())
            KagemushaP256Codec.requireRawLowSSignature(authenticator)
        } else require(payload.isEmpty() && authenticator.isEmpty())
        return Fields(operation, status, id, payload, authenticator)
    }

    private fun sha256(bytes: ByteArray) = MessageDigest.getInstance("SHA-256").digest(bytes)
}
