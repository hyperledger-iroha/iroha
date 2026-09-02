package org.hyperledger.iroha.sdk.offline

import java.io.File
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

class KagemushaDeviceLifecycleBridgeV1Test {
    @Test
    fun `unsupported devices remain online only and never execute`() {
        val bridge = KagemushaDeviceLifecycleBridgeV1.onlineOnly()
        assertEquals(KagemushaDeviceLifecycleBridgeV1.Availability.ONLINE_ONLY, bridge.availability)
        assertEquals(null, bridge.capabilities())
        assertFailsWith<IllegalStateException> {
            bridge.execute(
                KagemushaDeviceLifecycleBridgeV1.Operation.PREPARE_EXACT_NEXT_TRANSITION,
                fixed(0x11, 32),
                byteArrayOf(1),
            )
        }
    }

    @Test
    fun `exact capabilities unlock all journal and outbox operations`() {
        val endpoint = FakeEndpoint()
        val bridge = KagemushaDeviceLifecycleBridgeV1.withEndpointForTests(endpoint)
        assertEquals(KagemushaDeviceLifecycleBridgeV1.Availability.AVAILABLE, bridge.availability)
        assertContentEquals(fixed(0x22, 32), bridge.capabilities()!!.hardwarePolicyId())
        assertContentEquals(
            byteArrayOf(0xff.toByte(), 0xff.toByte(), 0x00, 0x00),
            endpoint.capabilityFrame.copyOfRange(12, 16),
        )
        assertEquals(
            (1..24).toList(),
            KagemushaDeviceLifecycleBridgeV1.Operation.values().map { it.code },
        )
        assertEquals(
            listOf(
                "READ_ACTIVE_HARDWARE_CREDENTIAL",
                "PREPARE_ACCEPTANCE_INTENT_AUTHORIZATION",
                "RECOVER_ACCEPTANCE_INTENT_AUTHORIZATION",
                "VERIFY_AUTHORIZATION_RESERVE_INBOX_AND_ISSUE_ACCEPTANCE_TICKET",
                "RECOVER_ACCEPTANCE_TICKET",
                "STAGE_INBOUND_PAYMENT",
                "RECOVER_STAGED_INBOUND_PAYMENT",
                "RECOVER_INBOUND_INBOX_PAGE",
                "PREPARE_EXACT_NEXT_TRANSITION",
                "RECOVER_PREPARED_TRANSITION",
                "ABANDON_UNCOMMITTED_PREPARED_TRANSITION",
                "COMMIT_VERIFIED_CANDIDATE",
                "RECOVER_TERMINAL_COMMIT_CERTIFICATE",
                "INSTALL_FINAL_COMMIT_WRAPPER",
                "RECOVER_INSTALLED_ENVELOPE_OR_STATE_PROOF",
                "SIGN_RECEIVE_ACKNOWLEDGEMENT",
                "RELEASE_OUTBOX_ENTRY",
                "READ_TRUSTED_TIME_OR_LEASE",
                "PREPARE_MINT_AUTHORIZATION",
                "RECOVER_MINT_AUTHORIZATION",
                "VERIFY_AUTHORIZATION_AND_STAGE_MINT_CREDIT",
                "FOLD_RECEIVE",
                "READ_PENDING_CREDIT_WATERMARK",
                "ROTATE_HARDWARE_EPOCH",
            ),
            KagemushaDeviceLifecycleBridgeV1.Operation.values().map { it.name },
        )
        assertEquals(
            (0 until 16).map { 1 shl it },
            KagemushaDeviceLifecycleBridgeV1.Capability.values().map { it.mask },
        )
        assertEquals(
            listOf(
                "EXACT_NEXT_PREDECESSOR_CONSUMPTION",
                "ONE_USE_SUCCESSOR_AUTHORIZATION",
                "ROLLBACK_RESISTANT_COUNTER_AND_JOURNAL",
                "SEALED_TRANSITION_RECOVERY",
                "ONE_USE_ACCEPTANCE_TICKETS",
                "DURABLE_INBOX_RESERVATION",
                "AUTHENTICATED_INBOUND_STAGING",
                "AUTHORITATIVE_REPLAY_ROOT_RECOVERY",
                "SENDER_OUTBOX_RESERVATION",
                "AUTHENTICATED_DURABLE_RETRY_OUTBOX",
                "ATOMIC_VERIFIED_CANDIDATE_COMMIT",
                "RECOVERABLE_TERMINAL_COMMIT_CERTIFICATE",
                "TRUSTED_TIME_OR_LEASE",
                "KAGEMUSHA_HARDWARE_EPOCH_ROTATION",
                "ROLLBACK_SAFE_COUNTER_ROLLOVER",
                "NO_SOFTWARE_FALLBACK",
            ),
            KagemushaDeviceLifecycleBridgeV1.Capability.values().map { it.name },
        )
        assertEquals(
            (0..10).toList(),
            KagemushaDeviceLifecycleBridgeV1.Status.values().map { it.code },
        )
        assertEquals(
            listOf(
                "SUCCESS",
                "UNAVAILABLE",
                "STALE_OR_CONCURRENT",
                "BINDING_MISMATCH",
                "TRUSTED_TIME_REJECTED",
                "REJECTED",
                "MISSING",
                "CONFLICT",
                "CORRUPT",
                "MALFORMED_REQUEST",
                "RECOVERY_REQUIRED",
            ),
            KagemushaDeviceLifecycleBridgeV1.Status.values().map { it.name },
        )

        for (operation in KagemushaDeviceLifecycleBridgeV1.Operation.values()) {
            endpoint.operation = operation
            val result = bridge.execute(operation, fixed(0x11, 32), byteArrayOf(1, 2, 3))
            assertEquals(KagemushaDeviceLifecycleBridgeV1.Status.SUCCESS, result.status)
            assertContentEquals(byteArrayOf(4, 5), result.payload())
            assertContentEquals(fixed(0x44, 64), result.authenticator())
            assertEquals(true, endpoint.lastCommand!!.all { it == 0.toByte() })
            assertEquals(true, endpoint.lastResponse!!.all { it == 0.toByte() })
        }
    }

    @Test
    fun `command framing is canonical and hard rejects old bridge versions`() {
        val encoded = KagemushaDeviceLifecycleBridgeV1.Codec.encodeCommand(
            KagemushaDeviceLifecycleBridgeV1.Operation.STAGE_INBOUND_PAYMENT,
            fixed(0x11, 32),
            byteArrayOf(1, 2, 3),
        )
        assertEquals(
            "494b474d4a434d3101000600" +
                "11".repeat(32) +
                "03000000" +
                "039058c6f2c0cb492c533b0a4d14ef77cc0f78abccced5287d84a1a2011cfb81" +
                "010203",
            encoded.toHex(),
        )

        for (retiredVersion in listOf(4, 5)) {
            val response = KagemushaDeviceLifecycleBridgeV1.Codec.encodeResponseForTests(
                KagemushaDeviceLifecycleBridgeV1.Operation.STAGE_INBOUND_PAYMENT,
                KagemushaDeviceLifecycleBridgeV1.Status.SUCCESS,
                fixed(0x11, 32),
                byteArrayOf(4),
                fixed(0x44, 64),
            )
            response[8] = retiredVersion.toByte()
            assertFailsWith<IllegalArgumentException> {
                KagemushaDeviceLifecycleBridgeV1.Codec.decodeResponse(
                    response,
                    KagemushaDeviceLifecycleBridgeV1.Operation.STAGE_INBOUND_PAYMENT,
                    fixed(0x11, 32),
                )
            }
        }

        for (unknownOperation in listOf(0, 25)) {
            val response = KagemushaDeviceLifecycleBridgeV1.Codec.encodeResponseForTests(
                KagemushaDeviceLifecycleBridgeV1.Operation.STAGE_INBOUND_PAYMENT,
                KagemushaDeviceLifecycleBridgeV1.Status.SUCCESS,
                fixed(0x11, 32),
                byteArrayOf(4),
                fixed(0x44, 64),
            )
            response[10] = unknownOperation.toByte()
            assertFailsWith<IllegalArgumentException> {
                KagemushaDeviceLifecycleBridgeV1.Codec.decodeResponse(
                    response,
                    KagemushaDeviceLifecycleBridgeV1.Operation.STAGE_INBOUND_PAYMENT,
                    fixed(0x11, 32),
                )
            }
        }

        val unknownStatus = KagemushaDeviceLifecycleBridgeV1.Codec.encodeResponseForTests(
            KagemushaDeviceLifecycleBridgeV1.Operation.STAGE_INBOUND_PAYMENT,
            KagemushaDeviceLifecycleBridgeV1.Status.SUCCESS,
            fixed(0x11, 32),
            byteArrayOf(4),
            fixed(0x44, 64),
        )
        unknownStatus[11] = 11
        assertFailsWith<IllegalArgumentException> {
            KagemushaDeviceLifecycleBridgeV1.Codec.decodeResponse(
                unknownStatus,
                KagemushaDeviceLifecycleBridgeV1.Operation.STAGE_INBOUND_PAYMENT,
                fixed(0x11, 32),
            )
        }

        val recoveryRequired = KagemushaDeviceLifecycleBridgeV1.Codec.encodeResponseForTests(
            KagemushaDeviceLifecycleBridgeV1.Operation.RECOVER_TERMINAL_COMMIT_CERTIFICATE,
            KagemushaDeviceLifecycleBridgeV1.Status.RECOVERY_REQUIRED,
            fixed(0x11, 32),
            byteArrayOf(),
            byteArrayOf(),
        )
        assertEquals(
            KagemushaDeviceLifecycleBridgeV1.Status.RECOVERY_REQUIRED,
            KagemushaDeviceLifecycleBridgeV1.Codec.decodeResponse(
                recoveryRequired,
                KagemushaDeviceLifecycleBridgeV1.Operation.RECOVER_TERMINAL_COMMIT_CERTIFICATE,
                fixed(0x11, 32),
            ).status,
        )
    }

    @Test
    fun `partial capabilities and unauthenticated success fail closed`() {
        for (featureBit in 0 until 16) {
            val partial = FakeEndpoint()
            val capabilities = partial.capabilities()
            val byteIndex = 12 + featureBit / 8
            capabilities[byteIndex] =
                (capabilities[byteIndex].toInt() and (1 shl (featureBit % 8)).inv()).toByte()
            partial.capabilityFrame = capabilities
            assertFailsWith<IllegalArgumentException>("accepted missing feature bit $featureBit") {
                KagemushaDeviceLifecycleBridgeV1.withEndpointForTests(partial)
            }
        }

        val unknownFeature = FakeEndpoint()
        val capabilities = unknownFeature.capabilities()
        capabilities[14] = 1
        unknownFeature.capabilityFrame = capabilities
        assertFailsWith<IllegalArgumentException> {
            KagemushaDeviceLifecycleBridgeV1.withEndpointForTests(unknownFeature)
        }

        val endpoint = FakeEndpoint()
        endpoint.authenticator = ByteArray(64)
        val bridge = KagemushaDeviceLifecycleBridgeV1.withEndpointForTests(endpoint)
        assertFailsWith<IllegalArgumentException> {
            bridge.execute(
                KagemushaDeviceLifecycleBridgeV1.Operation.RECOVER_TERMINAL_COMMIT_CERTIFICATE,
                fixed(0x11, 32),
                byteArrayOf(1),
            )
        }
    }

    @Test
    fun `response decoder transfers one owned result and retains no frame copy`() {
        val source =
            generateSequence(File(".").canonicalFile) { it.parentFile }
                .flatMap { root ->
                    sequenceOf(
                        File(
                            root,
                            "src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaDeviceLifecycleBridgeV1.kt",
                        ),
                        File(
                            root,
                            "kotlin/client-android/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaDeviceLifecycleBridgeV1.kt",
                        ),
                    )
                }
                .firstOrNull(File::isFile)
                ?.readText(Charsets.UTF_8)
                ?: error("cannot locate KagemushaDeviceLifecycleBridgeV1.kt")
        assertTrue(source.contains("ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN)"))
        assertFalse(source.contains("ByteBuffer.wrap(bytes.copyOf())"))
        assertTrue(source.contains("if (!transferred)"))
        assertTrue(source.contains("payload.fill(0)"))
        assertTrue(source.contains("authenticator.fill(0)"))
        assertTrue(source.contains("private val payloadBytes = payload"))
        assertTrue(source.contains("private val authenticatorBytes = authenticator"))
        assertFalse(source.contains("private val payloadBytes = payload.copyOf()"))
        assertFalse(source.contains("private val authenticatorBytes = authenticator.copyOf()"))
    }

    private class FakeEndpoint : KagemushaDeviceLifecycleBridgeV1.Endpoint {
        var operation = KagemushaDeviceLifecycleBridgeV1.Operation.RECOVER_TERMINAL_COMMIT_CERTIFICATE
        var authenticator = fixed(0x44, 64)
        var lastCommand: ByteArray? = null
        var lastResponse: ByteArray? = null
        var capabilityFrame = KagemushaDeviceLifecycleBridgeV1.Codec.encodeCapabilitiesForTests(
            1,
            fixed(0x22, 32),
            fixed(0x33, 32),
        )

        override fun capabilities(): ByteArray = capabilityFrame.copyOf()

        override fun execute(command: ByteArray): ByteArray {
            lastCommand = command
            assertContentEquals("IKGMJCM1".toByteArray(Charsets.US_ASCII), command.copyOfRange(0, 8))
            val requestId = command.copyOfRange(12, 44)
            return KagemushaDeviceLifecycleBridgeV1.Codec.encodeResponseForTests(
                operation,
                KagemushaDeviceLifecycleBridgeV1.Status.SUCCESS,
                requestId,
                byteArrayOf(4, 5),
                authenticator,
            ).also { lastResponse = it }
        }
    }

    companion object {
        private fun fixed(value: Int, count: Int): ByteArray = ByteArray(count) { value.toByte() }

        private fun ByteArray.toHex(): String = joinToString("") { "%02x".format(it.toInt() and 0xff) }
    }
}
