package org.hyperledger.iroha.sdk.offline

import org.junit.jupiter.api.Test
import kotlin.test.*

/** Callback refusal and pure framing only; inert bytes create no descriptor or Native authority. */
class KagemushaOrdinaryCurrentControlTransportBindingV1Test {
    @Test fun callerSelectedCallbacksNeverReceiveDescriptorOrRevokeOwner() {
        val native = CoreEndpoint()
        val bridge = KagemushaCoreCoordinatorBridgeV1.openEndpoint("/fixture/current-fi", native)
        val held = KagemushaOrdinaryCurrentControlTransportBindingV1(bridge)
        var dispatched = 0
        val callback = KagemushaOrdinaryNativeCurrentControlEndpointV1 { phase, handle, _, _ ->
            dispatched++
            response(phase, handle, if (phase == 3) emptyList() else listOf(byteArrayOf(1), ByteArray(64)))
        }
        // Same trusted loader, wrong class: even a correctly framed guessed correlation is refused.
        assertSame(KagemushaCoreCoordinatorBridgeV1::class.java.classLoader, callback.javaClass.classLoader)
        for (phase in 1..3) {
            val signed = if (phase == 3) byteArrayOf(2) else ByteArray(0)
            val world = if (phase == 3) byteArrayOf(3) else ByteArray(0)
            assertFailsWith<IllegalArgumentException> { held.invoke(callback, phase, signed, world) }
            // The internal Bridge entry cannot bypass the exact same production guard.
            assertFailsWith<IllegalArgumentException> { bridge.invokeOrdinaryCurrentControl(callback, phase, signed, world) }
            assertEquals(0, dispatched)
            assertEquals(0, native.closes)
            held.requireOpen()
        }
        held.revoke()
        assertEquals(1, native.closes)
        assertFails { held.requireOpen() }
    }

    @Test fun invalidOfferedOriginalsRefuseWithoutDispatchOrRevocation() {
        val native = CoreEndpoint()
        val bridge = KagemushaCoreCoordinatorBridgeV1.openEndpoint("/fixture/current-fi", native)
        val held = KagemushaOrdinaryCurrentControlTransportBindingV1(bridge)
        var dispatched = 0
        val callback = KagemushaOrdinaryNativeCurrentControlEndpointV1 { _, _, _, _ -> dispatched++; null }
        assertFailsWith<IllegalArgumentException> { held.invoke(callback, 0) }
        assertFailsWith<IllegalArgumentException> { held.invoke(callback, 1, byteArrayOf(1)) }
        assertFailsWith<IllegalArgumentException> { held.invoke(callback, 3, ByteArray(0), byteArrayOf(1)) }
        assertFailsWith<IllegalArgumentException> { held.invoke(callback, 3, byteArrayOf(1), ByteArray(0)) }
        assertEquals(0, dispatched)
        assertEquals(0, native.closes)
        held.requireOpen()
        bridge.close()
    }

    @Test fun pureCorrelationValidatorCopiesFieldsAndRequiresExactPhaseShape() {
        val request = byteArrayOf(1, 2)
        val signing = domain() + request
        val first = ordinaryCurrentControlResponseFieldsV1(1, 13L, response(1, 13L, listOf(request, signing)))
        request.fill(0); signing.fill(0)
        assertContentEquals(byteArrayOf(1, 2), first[0])
        assertContentEquals(domain() + byteArrayOf(1, 2), first[1])
        val signature = ByteArray(64) { 7 }
        val second = ordinaryCurrentControlResponseFieldsV1(2, 13L, response(2, 13L, listOf(byteArrayOf(3), signature)))
        signature.fill(0)
        assertContentEquals(ByteArray(64) { 7 }, second[1])
        assertTrue(ordinaryCurrentControlResponseFieldsV1(3, 13L, response(3, 13L, emptyList())).isEmpty())
    }

    @Test fun pureCorrelationValidatorRejectsIdentityAndFramingSubstitutions() {
        val fields = listOf(byteArrayOf(1), ByteArray(64))
        val good = response(2, 13L, fields)
        val malformed = listOf(
            response(2, 14L, fields),
            response(1, 13L, fields),
            good.copyOf().also { it[0] = byteArrayOf(2, 0) },
            good.copyOf().also { it[2] = byteArrayOf(13) },
            response(2, 13L, fields + byteArrayOf(9)),
            response(2, 13L, listOf(byteArrayOf(1))),
        )
        for (offered in malformed) assertFailsWith<IllegalArgumentException> {
            ordinaryCurrentControlResponseFieldsV1(2, 13L, offered)
        }
        assertFailsWith<IllegalArgumentException> { ordinaryCurrentControlResponseFieldsV1(0, 13L, good) }
        assertFailsWith<IllegalArgumentException> { ordinaryCurrentControlResponseFieldsV1(2, 0L, good) }
        assertFailsWith<IllegalArgumentException> { ordinaryCurrentControlResponseFieldsV1(3, 13L, response(3, 13L, fields)) }
    }

    @Test fun pureCorrelationValidatorRejectsChangedSigningSubjectAndUnboundedFields() {
        val oversized = ByteArray(KagemushaOrdinaryCurrentControlHttpCodecV1.MAXIMUM_REQUEST_BYTES + 1)
        for (fields in listOf(listOf(ByteArray(0), ByteArray(64)), listOf(oversized, ByteArray(64)),
            listOf(byteArrayOf(1), ByteArray(63)))) {
            assertFailsWith<IllegalArgumentException> { ordinaryCurrentControlResponseFieldsV1(2, 13L, response(2, 13L, fields)) }
        }
        for (signing in listOf(domain() + byteArrayOf(2), "different-domain".toByteArray() + byteArrayOf(1))) {
            assertFailsWith<IllegalArgumentException> {
                ordinaryCurrentControlResponseFieldsV1(1, 13L, response(1, 13L, listOf(byteArrayOf(1), signing)))
            }
        }
    }

    private class CoreEndpoint : KagemushaCoreCoordinatorEndpointV1 {
        var closes = 0
        override fun contract() = intArrayOf(2, 25, 3, 6, 54, 8, 7, 22, 16, 0xffff, 1, 21)
        override fun install(storagePath: String) = 0
        override fun open(storagePath: String) = 13L
        override fun invoke(handle: Long, method: Int, fields: Array<ByteArray>): Array<ByteArray>? =
            error("No generic financial or Bootstrap method")
        override fun close(handle: Long): Int { assertEquals(13L, handle); closes++; return 0 }
    }
    companion object {
        private fun domain() = "iroha:kagemusha:v1:ordinary-current-fi-control-request\u0000".toByteArray(Charsets.US_ASCII)
        private fun response(phase: Int, correlation: Long, fields: List<ByteArray>) =
            (listOf(byteArrayOf(1, 0), byteArrayOf(phase.toByte()),
                ByteArray(8) { (correlation ushr (it * 8)).toByte() }) + fields).toTypedArray()
    }
}
