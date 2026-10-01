// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertFailsWith

/** Structural byte diagnostics only; these fixtures do not assert device signature validity. */
class KagemushaDeviceResponseFrameV1Test {
    private val id = ByteArray(32) { 7 }
    private val payload = byteArrayOf(3, 4)
    private val signature = ByteArray(64).also { it[31] = 1; it[63] = 1 }

    @Test fun `complete original success is defensively retained without reconstructing it`() {
        val frame = structuralDeviceResponseFrame(7, id, payload, signature)
        val response = KagemushaAuthenticatedDeviceResponseV1(7, KagemushaAuthenticatedDeviceStatusV1.SUCCESS,
            payload, signature, frame)
        val original = frame.copyOf()
        frame.fill(0)
        response.canonicalResponseFrame().fill(0)
        assertContentEquals(original, response.canonicalResponseFrame())
        assertContentEquals(original, KagemushaDeviceResponseFrameV1.requireSuccessShape(original, 7, id))
    }

    @Test fun `inner reply and signed operation8 cannot substitute the operation7 original`() {
        assertFailsWith<IllegalArgumentException> { KagemushaDeviceResponseFrameV1.requireSuccessShape(payload, 7, id) }
        assertFailsWith<IllegalArgumentException> {
            KagemushaDeviceResponseFrameV1.requireSuccessShape(structuralDeviceResponseFrame(8, id, payload, signature), 7, id)
        }
    }

    @Test fun `original success rejects a different request status payload or authenticator`() {
        val frame = structuralDeviceResponseFrame(7, id, payload, signature)
        assertFailsWith<IllegalArgumentException> { KagemushaDeviceResponseFrameV1.requireSuccessShape(frame, 7, ByteArray(32) { 8 }) }
        assertFailsWith<IllegalArgumentException> {
            KagemushaAuthenticatedDeviceResponseV1(7, KagemushaAuthenticatedDeviceStatusV1.SUCCESS, byteArrayOf(9), signature, frame)
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaAuthenticatedDeviceResponseV1(7, KagemushaAuthenticatedDeviceStatusV1.SUCCESS, payload,
                signature.copyOf().also { it[63] = 2 }, frame)
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaDeviceResponseFrameV1.requireSuccessShape(structuralDeviceResponseFrame(7, id, byteArrayOf(), byteArrayOf(), 10), 7, id)
        }
    }

    @Test fun `exact original rejects invalid lengths checksums tails signatures and schema`() {
        val frame = structuralDeviceResponseFrame(7, id, payload, signature)
        listOf(frame + 0, frame.copyOf(frame.size - 1), ByteArray(KagemushaDeviceResponseFrameV1.MAXIMUM_BYTES + 1),
            frame.copyOf().also { it[8] = 2 }, frame.copyOf().also { it[44] = 127 },
            frame.copyOf().also { it[it.lastIndex] = 2 },
            structuralDeviceResponseFrame(7, id, payload, ByteArray(64)),
        ).forEach { invalid -> assertFailsWith<IllegalArgumentException> { KagemushaDeviceResponseFrameV1.requireSuccessShape(invalid, 7, id) } }
    }
}

internal fun structuralDeviceResponseFrame(operation: Int, id: ByteArray, payload: ByteArray,
    signature: ByteArray, status: Int = 0): ByteArray = ByteBuffer.allocate(116 + payload.size + signature.size)
    .order(ByteOrder.LITTLE_ENDIAN).put("IKGMJRS1".toByteArray(Charsets.US_ASCII)).putShort(1)
    .put(operation.toByte()).put(status.toByte()).put(id).putInt(payload.size).putInt(signature.size)
    .put(MessageDigest.getInstance("SHA-256").digest(payload)).put(MessageDigest.getInstance("SHA-256").digest(signature))
    .put(payload).put(signature).array()
