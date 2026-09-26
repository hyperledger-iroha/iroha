// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.io.IOException
import java.security.MessageDigest
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

private typealias Operation = KagemushaDeviceLifecycleBridgeV1.Operation

/**
 * Exercises the ABI-24 APDU recovery seam with a deterministic test-only applet model.
 *
 * The model has no secure storage or signing key and grants no device qualification. A
 * provisioned eSE must independently satisfy the same lost-result and competing-successor
 * contract with canonical Core commands and physically verified attestation.
 */
class KagemushaProvisionedEseTransportContractV1Test {
    @Test
    fun `lost commit result recovers original outcome and rejects second successor`() {
        val predecessor = ByteArray(32) { 0x31 }
        val firstId = ByteArray(32) { 0x41 }
        val secondId = ByteArray(32) { 0x42 }
        val model = AtomicSuccessorAppletModel(predecessor, firstId)
        val endpoint = KagemushaSecureElementApduEndpointV1(model.openChannel())
        val codec = KagemushaDeviceLifecycleBridgeV1.Codec

        val firstPreparation = codec.encodeCommand(
            Operation.PREPARE_EXACT_NEXT_TRANSITION,
            firstId,
            predecessor + byteArrayOf(1),
        )
        val prepared = codec.decodeResponse(
            endpoint.execute(firstPreparation),
            Operation.PREPARE_EXACT_NEXT_TRANSITION,
            firstId,
        )
        assertEquals(KagemushaDeviceLifecycleBridgeV1.Status.SUCCESS, prepared.status)

        val competingPreparation = codec.encodeCommand(
            Operation.PREPARE_EXACT_NEXT_TRANSITION,
            secondId,
            predecessor + byteArrayOf(3),
        )
        val earlyRefusal = codec.decodeResponse(
            endpoint.execute(competingPreparation),
            Operation.PREPARE_EXACT_NEXT_TRANSITION,
            secondId,
        )
        assertEquals(KagemushaDeviceLifecycleBridgeV1.Status.CONFLICT, earlyRefusal.status)
        assertTrue(earlyRefusal.payload().isEmpty())
        assertTrue(earlyRefusal.authenticator().isEmpty())

        val commit = codec.encodeCommand(
            Operation.COMMIT_VERIFIED_CANDIDATE_AND_SIGN_TERMINAL,
            firstId,
            predecessor + byteArrayOf(2),
        )
        model.dropNextCommitMetadata = true
        assertFailsWith<IOException> { endpoint.execute(commit) }
        assertEquals(1, model.durableCommitCount)
        assertEquals(1, model.transportAbortCount)

        val recovery = codec.encodeCommand(
            Operation.RECOVER_TERMINAL_OUTCOME,
            firstId,
            predecessor,
        )
        assertFailsWith<IllegalStateException> { endpoint.execute(recovery) }
        val recoveryEndpoint = KagemushaSecureElementApduEndpointV1(model.openChannel())
        val recovered = codec.decodeResponse(
            recoveryEndpoint.execute(recovery),
            Operation.RECOVER_TERMINAL_OUTCOME,
            firstId,
        )
        assertEquals(KagemushaDeviceLifecycleBridgeV1.Status.SUCCESS, recovered.status)
        assertContentEquals(model.originalOutcome, recovered.payload())

        val retry = codec.decodeResponse(
            recoveryEndpoint.execute(commit),
            Operation.COMMIT_VERIFIED_CANDIDATE_AND_SIGN_TERMINAL,
            firstId,
        )
        assertEquals(KagemushaDeviceLifecycleBridgeV1.Status.SUCCESS, retry.status)
        assertContentEquals(recovered.payload(), retry.payload())
        assertEquals(1, model.durableCommitCount)

        val changedCandidate = codec.encodeCommand(
            Operation.COMMIT_VERIFIED_CANDIDATE_AND_SIGN_TERMINAL,
            firstId,
            predecessor + byteArrayOf(4),
        )
        val changedResult = codec.decodeResponse(
            recoveryEndpoint.execute(changedCandidate),
            Operation.COMMIT_VERIFIED_CANDIDATE_AND_SIGN_TERMINAL,
            firstId,
        )
        assertEquals(KagemushaDeviceLifecycleBridgeV1.Status.CONFLICT, changedResult.status)
        assertTrue(changedResult.payload().isEmpty())
        assertTrue(changedResult.authenticator().isEmpty())

        val refused = codec.decodeResponse(
            recoveryEndpoint.execute(competingPreparation),
            Operation.PREPARE_EXACT_NEXT_TRANSITION,
            secondId,
        )
        assertEquals(KagemushaDeviceLifecycleBridgeV1.Status.CONFLICT, refused.status)
        assertTrue(refused.payload().isEmpty())
        assertTrue(refused.authenticator().isEmpty())
        assertEquals(1, model.durableCommitCount)
    }

    private class AtomicSuccessorAppletModel(
        private val predecessor: ByteArray,
        private val firstId: ByteArray,
    ) {
        var originalOutcome = ByteArray(0)
            private set
        var dropNextCommitMetadata = false
        var durableCommitCount = 0
            private set
        var transportAbortCount = 0
            private set

        private var expectedLength = 0
        private var expectedDigest = ByteArray(0)
        private var commandBytes = ByteArray(0)
        private var responseBytes = ByteArray(0)
        private var prepared = false
        private var committed = false
        private var originalCommitDigest = ByteArray(0)

        fun openChannel(): KagemushaSecureElementApduEndpointV1.Channel =
            object : KagemushaSecureElementApduEndpointV1.Channel {
                private var closed = false

                override fun transmit(command: ByteArray): ByteArray {
                    check(!closed) { "selected applet channel is closed" }
                    return this@AtomicSuccessorAppletModel.transmit(command)
                }

                override fun close() {
                    closed = true
                }
            }

        private fun transmit(command: ByteArray): ByteArray {
            require(command.size >= 4 && command[0] == 0x80.toByte())
            return when (command[1].toInt() and 0xff) {
                0x12 -> {
                    require(command.size == 41 && command[4] == 36.toByte())
                    expectedLength = readU32Le(command, 5)
                    expectedDigest = command.copyOfRange(9, 41)
                    commandBytes = ByteArray(0)
                    success()
                }
                0x13 -> {
                    require(command.size >= 6)
                    val index = ((command[2].toInt() and 0xff) shl 8) or
                        (command[3].toInt() and 0xff)
                    assertEquals(commandBytes.size / 224, index)
                    val count = command[4].toInt() and 0xff
                    assertEquals(count + 5, command.size)
                    commandBytes += command.copyOfRange(5, command.size)
                    success()
                }
                0x14 -> {
                    assertEquals(expectedLength, commandBytes.size)
                    assertContentEquals(expectedDigest, sha256(commandBytes))
                    responseBytes = handleCommand(commandBytes)
                    if (dropNextCommitMetadata) {
                        dropNextCommitMetadata = false
                        throw IOException("simulated result loss after durable commit")
                    }
                    success(u32Le(responseBytes.size) + sha256(responseBytes))
                }
                0x15 -> {
                    val index = ((command[2].toInt() and 0xff) shl 8) or
                        (command[3].toInt() and 0xff)
                    val requested = command[4].toInt() and 0xff
                    val count = if (requested == 0) 256 else requested
                    val offset = index * 224
                    require(offset + count <= responseBytes.size)
                    success(responseBytes.copyOfRange(offset, offset + count))
                }
                0x16 -> {
                    transportAbortCount += 1
                    commandBytes = ByteArray(0)
                    responseBytes = ByteArray(0)
                    success()
                }
                else -> error("unexpected APDU instruction")
            }
        }

        private fun handleCommand(frame: ByteArray): ByteArray {
            require(frame.size >= 81)
            assertContentEquals("IKGMJCM1".toByteArray(Charsets.US_ASCII), frame.copyOfRange(0, 8))
            assertEquals(1, readU16Le(frame, 8))
            assertEquals(0, frame[11].toInt())
            val operationCode = frame[10].toInt() and 0xff
            val operation = KagemushaDeviceLifecycleBridgeV1.Operation.values()
                .single { it.code == operationCode }
            val id = frame.copyOfRange(12, 44)
            val payloadLength = readU32Le(frame, 44)
            assertEquals(frame.size - 80, payloadLength)
            val payload = frame.copyOfRange(80, frame.size)
            assertContentEquals(frame.copyOfRange(48, 80), sha256(payload))
            assertContentEquals(predecessor, payload.copyOfRange(0, 32))
            val originalId = id.contentEquals(firstId)
            val status = when (operation) {
                KagemushaDeviceLifecycleBridgeV1.Operation.PREPARE_EXACT_NEXT_TRANSITION -> {
                    if (!originalId || committed) KagemushaDeviceLifecycleBridgeV1.Status.CONFLICT
                    else {
                        prepared = true
                        KagemushaDeviceLifecycleBridgeV1.Status.SUCCESS
                    }
                }
                KagemushaDeviceLifecycleBridgeV1.Operation.COMMIT_VERIFIED_CANDIDATE_AND_SIGN_TERMINAL -> {
                    val digest = sha256(payload)
                    if (!originalId || !prepared ||
                        (committed && !digest.contentEquals(originalCommitDigest))
                    ) {
                        KagemushaDeviceLifecycleBridgeV1.Status.CONFLICT
                    } else {
                        if (!committed) {
                            originalCommitDigest = digest
                            committed = true
                            durableCommitCount += 1
                            originalOutcome = sha256(frame) +
                                sha256(predecessor + firstId + byteArrayOf(durableCommitCount.toByte()))
                        }
                        KagemushaDeviceLifecycleBridgeV1.Status.SUCCESS
                    }
                }
                KagemushaDeviceLifecycleBridgeV1.Operation.RECOVER_TERMINAL_OUTCOME ->
                    if (originalId && committed) KagemushaDeviceLifecycleBridgeV1.Status.SUCCESS
                    else KagemushaDeviceLifecycleBridgeV1.Status.MISSING
                else -> error("unexpected lifecycle operation")
            }
            val result = if (status == KagemushaDeviceLifecycleBridgeV1.Status.SUCCESS) {
                if (operation == KagemushaDeviceLifecycleBridgeV1.Operation.PREPARE_EXACT_NEXT_TRANSITION) {
                    sha256(payload)
                } else {
                    originalOutcome
                }
            } else {
                ByteArray(0)
            }
            val authenticator = if (status == KagemushaDeviceLifecycleBridgeV1.Status.SUCCESS) {
                ByteArray(64) { 0x5a.toByte() }
            } else {
                ByteArray(0)
            }
            return KagemushaDeviceLifecycleBridgeV1.Codec.encodeResponseForTests(
                operation, status, id, result, authenticator,
            )
        }

        private fun success(data: ByteArray = ByteArray(0)): ByteArray =
            data + byteArrayOf(0x90.toByte(), 0)
    }

    companion object {
        private fun sha256(bytes: ByteArray): ByteArray =
            MessageDigest.getInstance("SHA-256").digest(bytes)

        private fun u32Le(value: Int): ByteArray = ByteArray(4) { index ->
            (value ushr (index * 8)).toByte()
        }

        private fun readU16Le(bytes: ByteArray, offset: Int): Int =
            (bytes[offset].toInt() and 0xff) or ((bytes[offset + 1].toInt() and 0xff) shl 8)

        private fun readU32Le(bytes: ByteArray, offset: Int): Int =
            (0 until 4).fold(0) { value, index ->
                value or ((bytes[offset + index].toInt() and 0xff) shl (index * 8))
            }
    }
}
