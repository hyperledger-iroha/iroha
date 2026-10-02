// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import org.junit.jupiter.api.Test
import kotlin.test.*

/** Managed binding order only; scripted results do not authenticate an Android Application. */
class KagemushaOrdinaryApplicationBindingGateV1Test {
    @Test fun sameOriginalRechecksNativeWithoutAnotherProbeAndForeignIdentityCannotBind() {
        val gate = KagemushaOrdinaryApplicationBindingGateV1<Any>()
        val original = Any()
        var probes = 0
        var bindings = 0
        repeat(2) { gate.bind(original, { probes++ }) { bindings++; true } }
        assertEquals(1, probes)
        assertEquals(2, bindings)
        assertFailsWith<IllegalStateException> {
            gate.bind(Any(), { error("No foreign probe") }) { error("No foreign binding") }
        }
        assertEquals(2, bindings)
    }

    @Test fun failedProbeOrLostNativeResultConsumesBindingWithoutRetry() {
        for (failProbe in listOf(false, true)) {
            val gate = KagemushaOrdinaryApplicationBindingGateV1<Any>()
            val original = Any()
            var probes = 0
            var bindings = 0
            assertFailsWith<IllegalStateException> {
                gate.bind(original, { probes++; if (failProbe) error("Probe unavailable") }) {
                    bindings++; error("Native binding result lost")
                }
            }
            assertFailsWith<IllegalStateException> {
                gate.bind(original, { error("No retry probe") }) { error("No retry binding") }
            }
            assertEquals(1, probes)
            assertEquals(if (failProbe) 0 else 1, bindings)
        }
    }

    @Test fun refusedRetainedRecheckPermanentlyDeniesOriginal() {
        val gate = KagemushaOrdinaryApplicationBindingGateV1<Any>()
        val original = Any()
        gate.bind(original, {}) { true }
        assertFailsWith<IllegalStateException> { gate.bind(original, { error("No second probe") }) { false } }
        assertFailsWith<IllegalStateException> { gate.bind(original, {}) { error("No rebind after refusal") } }
    }
}
