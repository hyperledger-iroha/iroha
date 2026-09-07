package org.hyperledger.iroha.sdk.offline

import kotlin.test.assertEquals
import kotlin.test.assertContentEquals
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

class KagemushaDeviceLifecycleBridgeV1Test {
    @Test
    fun `exact frame survives input and accessor mutation`() {
        val operation = KagemushaDeviceLifecycleBridgeV1.Operation.READ_ACTIVE_HARDWARE_CREDENTIAL
        val id = ByteArray(32) { 0x11 }
        val expected = KagemushaDeviceLifecycleBridgeV1.Codec.encodeResponseForTests(
            operation, KagemushaDeviceLifecycleBridgeV1.Status.SUCCESS, id,
            byteArrayOf(4, 5), ByteArray(64) { 0x44 },
        )
        val input = expected.copyOf()
        val result = KagemushaDeviceLifecycleBridgeV1.Codec.decodeResponse(input, operation, id)
        input.fill(0)
        result.canonicalResponseFrame().fill(0)
        assertContentEquals(expected, result.canonicalResponseFrame())
        assertContentEquals(byteArrayOf(4, 5), result.payload())
    }

    @Test
    fun `verification and result share the exact decoded frame despite cleared transport buffer`() {
        val operation = KagemushaDeviceLifecycleBridgeV1.Operation.READ_ACTIVE_HARDWARE_CREDENTIAL
        val id = ByteArray(32) { 0x11 }
        val expected = KagemushaDeviceLifecycleBridgeV1.Codec.encodeResponseForTests(
            operation, KagemushaDeviceLifecycleBridgeV1.Status.SUCCESS, id,
            byteArrayOf(4, 5), ByteArray(64) { 0x44 },
        )
        val transportBuffer = expected.copyOf()
        // A framing/ownership fixture only; this callback supplies no hardware proof authority.
        val endpoint = object : KagemushaDeviceLifecycleBridgeV1.Endpoint {
            override fun capabilities() = KagemushaDeviceLifecycleBridgeV1.Codec.encodeCapabilitiesForTests(
                1, ByteArray(32) { 0x22 }, ByteArray(32) { 0x33 },
            )
            override fun execute(command: ByteArray) = transportBuffer
            override fun verifyCommandResponse(
                response: ByteArray, canonicalCommand: ByteArray,
                operation: KagemushaDeviceLifecycleBridgeV1.Operation, requestId: ByteArray,
                hardwarePolicyId: ByteArray, qualificationReportDigest: ByteArray,
                acceptedDevicePublicKey: ByteArray?,
            ): Boolean {
                transportBuffer.fill(0)
                assertContentEquals(expected, response)
                return true
            }
        }
        val result = KagemushaDeviceLifecycleBridgeV1.withEndpointForTests(endpoint)
            .executeAuthenticated(operation, id, byteArrayOf(1), null)
        assertTrue(transportBuffer.all { it == 0.toByte() })
        assertContentEquals(expected, result.canonicalResponseFrame())
    }

    @Test
    fun `native contract vector probe is bounded when linked`() {
        assertEquals(4 * 1024, KagemushaDeviceLifecycleBridgeV1.MAXIMUM_NATIVE_CONTRACT_VECTOR_BYTES)
        KagemushaDeviceLifecycleBridgeV1.nativeContractVector()?.let { vector ->
            assertTrue(vector.isNotEmpty())
            assertTrue(vector.size <= KagemushaDeviceLifecycleBridgeV1.MAXIMUM_NATIVE_CONTRACT_VECTOR_BYTES)
        }
    }

    @Test
    fun `device operations are contiguous and complete`() {
        val operations = KagemushaDeviceLifecycleBridgeV1.Operation.values()
        assertEquals((1..22).toList(), operations.map { it.code })
        assertEquals("STAGE_INBOUND_PAYMENT", operations[1].name)
        assertEquals("FOLD_RECEIVE_CREDIT", operations[16].name)
        assertEquals("CREATE_SIGNED_PAYMENT_REQUEST", operations.last().name)
    }

    @Test
    fun `hardware contract names receiver bound staging`() {
        val names = KagemushaDeviceLifecycleBridgeV1.Capability.values().map { it.name }
        assertTrue("RECEIVER_BOUND_CREDIT_COMMIT" in names)
        assertTrue("ROLLBACK_RESISTANT_ACCEPTED_CREDIT_INBOX" in names)
    }
}
