package org.hyperledger.iroha.sdk.offline

import org.junit.jupiter.api.Test
import kotlin.test.*

/** Inert lifecycle transport/framing only; no native root, loading or session is fabricated. */
class KagemushaOrdinaryRuntimeLifecycleV1Test {
    @Test fun lifecycleCallsOnlyPhaseFiveZeroIdAndEmptyOriginal() {
        var calls = 0
        revokeOrdinaryRuntimeSelectionV1(KagemushaOrdinaryNativeStartupEndpointV1 { phase, id, original ->
            calls++; assertEquals(5, phase); assertEquals(0L, id); assertTrue(original.isEmpty()); acknowledgement()
        })
        assertEquals(1, calls)
    }
    @Test fun unavailableRejectedAndLinkageRepliesNeverBecomeNoopSuccess() {
        var calls = 0
        assertFailsWith<IllegalStateException> { revokeOrdinaryRuntimeSelectionV1(KagemushaOrdinaryNativeStartupEndpointV1 { _, _, _ -> calls++; null }) }
        assertEquals(1, calls)
        assertFailsWith<IllegalStateException> { revokeOrdinaryRuntimeSelectionV1(KagemushaOrdinaryNativeStartupEndpointV1 { _, _, _ -> throw UnsatisfiedLinkError("inert missing Native startup") }) }
        assertFailsWith<IllegalStateException> { revokeOrdinaryRuntimeSelectionV1(KagemushaOrdinaryNativeStartupEndpointV1 { _, _, _ -> error("inert Native rejection") }) }
    }
    @Test fun lifecycleRequiresTheExactThreeFieldVersionPhaseAndZeroIdAcknowledgement() {
        val malformed = listOf(
            emptyArray<ByteArray>(), acknowledgement().take(2).toTypedArray(), acknowledgement() + byteArrayOf(1),
            acknowledgement().also { it[0] = byteArrayOf(2, 0) }, acknowledgement().also { it[0] = byteArrayOf(1) },
            acknowledgement().also { it[1] = byteArrayOf(4) }, acknowledgement().also { it[1] = byteArrayOf(5, 0) },
            acknowledgement().also { it[2] = ByteArray(7) }, acknowledgement().also { it[2] = ByteArray(8) { 1 } })
        for (response in malformed) assertFailsWith<IllegalArgumentException> {
            revokeOrdinaryRuntimeSelectionV1(KagemushaOrdinaryNativeStartupEndpointV1 { _, _, _ -> response })
        }
    }
    private fun acknowledgement() = arrayOf(byteArrayOf(1, 0), byteArrayOf(5), ByteArray(8))
}
