package org.hyperledger.iroha.sdk.offline

import java.io.File
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

class OfflineCashDeviceLifecycleBridgeV1Test {
    @Test
    fun `unsupported devices remain online only and never execute`() {
        val bridge = OfflineCashDeviceLifecycleBridgeV1.onlineOnly()
        assertEquals(OfflineCashDeviceLifecycleBridgeV1.Availability.ONLINE_ONLY, bridge.availability)
        assertEquals(null, bridge.capabilities())
        assertFailsWith<IllegalStateException> {
            bridge.execute(
                OfflineCashDeviceLifecycleBridgeV1.Operation.COMMIT_INTENT_EXACT_NEXT,
                fixed(0x11, 32),
                byteArrayOf(1),
            )
        }
    }

    @Test
    fun `production cannot be enabled by optional symbols or structural capabilities`() {
        val bridge = OfflineCashDeviceLifecycleBridgeV1.production()
        assertEquals(OfflineCashDeviceLifecycleBridgeV1.Availability.ONLINE_ONLY, bridge.availability)
        assertEquals(null, bridge.capabilities())

        val source = sourceText()
        val production = source.substringAfter(
            "fun production(): OfflineCashDeviceLifecycleBridgeV1",
        ).substringBefore("/** Explicit online-only instance")
        assertTrue(production.contains("= onlineOnly()"))
        assertFalse(production.contains("capabilities"))
        assertFalse(source.contains("NativeEndpoint"))
        assertFalse(source.contains("nativeCapabilitiesV1"))
        assertFalse(source.contains("nativeExecuteV1"))
        assertFalse(source.contains("System.loadLibrary"))
        assertTrue(source.contains("internal fun withEndpointForTests("))
    }

    @Test
    fun `exact capabilities unlock all journal and outbox operations`() {
        assertEquals(14, OfflineCashDeviceLifecycleBridgeV1.OPERATION_COUNT)
        assertEquals(96, OfflineCashDeviceLifecycleBridgeV1.CAPABILITY_FRAME_BYTES)
        assertEquals(0x1ff, OfflineCashDeviceLifecycleBridgeV1.REQUIRED_CAPABILITY_MASK)
        assertEquals(
            (1..OfflineCashDeviceLifecycleBridgeV1.OPERATION_COUNT).toList(),
            OfflineCashDeviceLifecycleBridgeV1.Operation.values().map { it.code },
        )
        assertEquals(
            (0..9).toList(),
            OfflineCashDeviceLifecycleBridgeV1.Status.values().map { it.code },
        )
        assertEquals(
            listOf(
                "SUCCESS",
                "UNAVAILABLE",
                "STALE_OR_CONCURRENT",
                "INTENT_MISMATCH",
                "TRUSTED_TIME_REJECTED",
                "POLICY_REJECTED",
                "MISSING",
                "CONFLICT",
                "CORRUPT",
                "MALFORMED_REQUEST",
            ),
            OfflineCashDeviceLifecycleBridgeV1.Status.values().map { it.name },
        )
        val endpoint = FakeEndpoint()
        assertEquals(
            OfflineCashDeviceLifecycleBridgeV1.CAPABILITY_FRAME_BYTES,
            endpoint.capabilityFrame.size,
        )
        val bridge = OfflineCashDeviceLifecycleBridgeV1.withEndpointForTests(endpoint)
        assertEquals(OfflineCashDeviceLifecycleBridgeV1.Availability.AVAILABLE, bridge.availability)
        assertContentEquals(fixed(0x22, 32), bridge.capabilities()!!.hardwarePolicyId())

        for (operation in OfflineCashDeviceLifecycleBridgeV1.Operation.values()) {
            endpoint.operation = operation
            val result = bridge.execute(operation, fixed(0x11, 32), byteArrayOf(1, 2, 3))
            assertEquals(OfflineCashDeviceLifecycleBridgeV1.Status.SUCCESS, result.status)
            assertContentEquals(byteArrayOf(4, 5), result.payload())
            assertContentEquals(fixed(0x44, 64), result.authenticator())
            assertEquals(true, endpoint.lastCommand!!.all { it == 0.toByte() })
            assertEquals(true, endpoint.lastResponse!!.all { it == 0.toByte() })
        }
    }

    @Test
    fun `command framing is canonical and hard rejects old bridge versions`() {
        val encoded = OfflineCashDeviceLifecycleBridgeV1.Codec.encodeCommand(
            OfflineCashDeviceLifecycleBridgeV1.Operation.CANCEL_EXPIRED_RECEIVE,
            fixed(0x11, 32),
            byteArrayOf(1, 2, 3),
        )
        assertEquals(
            "494f43464a434d3101000600" +
                "11".repeat(32) +
                "03000000" +
                "039058c6f2c0cb492c533b0a4d14ef77cc0f78abccced5287d84a1a2011cfb81" +
                "010203",
            encoded.toHex(),
        )

        for (retiredVersion in listOf(4, 5)) {
            val response = OfflineCashDeviceLifecycleBridgeV1.Codec.encodeResponseForTests(
                OfflineCashDeviceLifecycleBridgeV1.Operation.CANCEL_EXPIRED_RECEIVE,
                OfflineCashDeviceLifecycleBridgeV1.Status.SUCCESS,
                fixed(0x11, 32),
                byteArrayOf(4),
                fixed(0x44, 64),
            )
            response[8] = retiredVersion.toByte()
            assertFailsWith<IllegalArgumentException> {
                OfflineCashDeviceLifecycleBridgeV1.Codec.decodeResponse(
                    response,
                    OfflineCashDeviceLifecycleBridgeV1.Operation.CANCEL_EXPIRED_RECEIVE,
                    fixed(0x11, 32),
                )
            }
        }
    }

    @Test
    fun `partial capabilities and unauthenticated success fail closed`() {
        for (featureBit in 0 until 9) {
            val partial = FakeEndpoint()
            val capabilities = partial.capabilities()
            val byteIndex = 12 + featureBit / 8
            capabilities[byteIndex] =
                (capabilities[byteIndex].toInt() and (1 shl (featureBit % 8)).inv()).toByte()
            partial.capabilityFrame = capabilities
            assertFailsWith<IllegalArgumentException>("accepted missing feature bit $featureBit") {
                OfflineCashDeviceLifecycleBridgeV1.withEndpointForTests(partial)
            }
        }

        val endpoint = FakeEndpoint()
        endpoint.authenticator = ByteArray(64)
        val bridge = OfflineCashDeviceLifecycleBridgeV1.withEndpointForTests(endpoint)
        assertFailsWith<IllegalArgumentException> {
            bridge.execute(
                OfflineCashDeviceLifecycleBridgeV1.Operation.RECOVER_TERMINAL,
                fixed(0x11, 32),
                byteArrayOf(1),
            )
        }
    }

    @Test
    fun `response decoder transfers one owned result and retains no frame copy`() {
        val source = sourceText()
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

    private class FakeEndpoint : OfflineCashDeviceLifecycleBridgeV1.Endpoint {
        var operation = OfflineCashDeviceLifecycleBridgeV1.Operation.RECOVER_TERMINAL
        var authenticator = fixed(0x44, 64)
        var lastCommand: ByteArray? = null
        var lastResponse: ByteArray? = null
        var capabilityFrame = OfflineCashDeviceLifecycleBridgeV1.Codec.encodeCapabilitiesForTests(
            1,
            fixed(0x22, 32),
            fixed(0x33, 32),
        )

        override fun capabilities(): ByteArray = capabilityFrame.copyOf()

        override fun execute(command: ByteArray): ByteArray {
            lastCommand = command
            assertContentEquals("IOCFJCM1".toByteArray(Charsets.US_ASCII), command.copyOfRange(0, 8))
            val requestId = command.copyOfRange(12, 44)
            return OfflineCashDeviceLifecycleBridgeV1.Codec.encodeResponseForTests(
                operation,
                OfflineCashDeviceLifecycleBridgeV1.Status.SUCCESS,
                requestId,
                byteArrayOf(4, 5),
                authenticator,
            ).also { lastResponse = it }
        }
    }

    companion object {
        private fun sourceText(): String =
            generateSequence(File(".").canonicalFile) { it.parentFile }
                .flatMap { root ->
                    sequenceOf(
                        File(
                            root,
                            "src/main/java/org/hyperledger/iroha/sdk/offline/OfflineCashDeviceLifecycleBridgeV1.kt",
                        ),
                        File(
                            root,
                            "kotlin/client-android/src/main/java/org/hyperledger/iroha/sdk/offline/OfflineCashDeviceLifecycleBridgeV1.kt",
                        ),
                    )
                }
                .firstOrNull(File::isFile)
                ?.readText(Charsets.UTF_8)
                ?: error("cannot locate OfflineCashDeviceLifecycleBridgeV1.kt")

        private fun fixed(value: Int, count: Int): ByteArray = ByteArray(count) { value.toByte() }

        private fun ByteArray.toHex(): String = joinToString("") { "%02x".format(it.toInt() and 0xff) }
    }
}
