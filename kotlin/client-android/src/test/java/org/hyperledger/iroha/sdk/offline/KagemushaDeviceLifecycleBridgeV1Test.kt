package org.hyperledger.iroha.sdk.offline

import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

class KagemushaDeviceLifecycleBridgeV1Test {
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
