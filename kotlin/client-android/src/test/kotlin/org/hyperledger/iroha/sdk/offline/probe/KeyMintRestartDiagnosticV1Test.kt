// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.probe

import java.security.InvalidKeyException
import java.security.ProviderException
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertSame
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

class KeyMintRestartDiagnosticV1Test {
    private class PermanentlyInvalidated : InvalidKeyException()

    private fun observe(
        control: () -> Unit = {},
        lookup: () -> String? = { null },
        sign: (String) -> Unit = {},
        complete: () -> Unit,
    ): KeyMintRestartObservationV1 = completeKeyMintRestartDiagnosticV1(
        control, lookup, sign, { it is PermanentlyInvalidated }, complete,
    )

    @Test fun absentConsumedKeyIsAcceptedOnlyAfterSuccessfulControl() {
        val calls = mutableListOf<String>()
        val result = observe(
            control = { calls += "control" },
            lookup = { calls += "lookup"; null },
            sign = { error("absent key must not sign") },
            complete = { calls += "remove marker" },
        )
        assertEquals(KeyMintRestartObservationV1.ABSENT, result)
        assertEquals(listOf("control", "lookup", "remove marker"), calls)
    }

    @Test fun controlFailureRetainsMarkerAndDoesNotInspectConsumedKey() {
        for (failure in listOf(ProviderException("outage"), PermanentlyInvalidated())) {
            val observed = assertFailsWith<Exception> {
                observe(
                    control = { throw failure },
                    lookup = { error("control failure must stop lookup") },
                    complete = { error("control failure must retain marker") },
                )
            }
            assertSame(failure, observed)
        }
    }

    @Test fun onlyDirectPermanentInvalidationCompletesAnExistingConsumedKey() {
        for (duringLookup in listOf(true, false)) {
            var completed = false
            val result = observe(
                lookup = { if (duringLookup) throw PermanentlyInvalidated() else "key" },
                sign = { throw PermanentlyInvalidated() },
                complete = { completed = true },
            )
            assertEquals(KeyMintRestartObservationV1.PERMANENTLY_INVALIDATED, result)
            assertTrue(completed)
        }
    }

    @Test fun unrelatedLookupAndSigningFailuresRetainMarker() {
        for (duringLookup in listOf(true, false)) {
            for (failure in listOf(
                ProviderException("keystore unavailable"),
                InvalidKeyException("key may require authentication"),
                ProviderException("wrapped provider failure", PermanentlyInvalidated()),
                IllegalStateException("alias exists but key is absent"),
            )) {
                val observed = assertFailsWith<Exception> {
                    observe(
                        lookup = { if (duringLookup) throw failure else "key" },
                        sign = { throw failure },
                        complete = { error("unexpected failure must retain marker") },
                    )
                }
                assertSame(failure, observed)
            }
        }
    }

    @Test fun successfulSecondSignatureFailsWithoutRemovingMarker() {
        val failure = assertFailsWith<IllegalStateException> {
            observe(lookup = { "key" }, sign = {},
                complete = { error("second signature must retain marker") })
        }
        assertEquals("consumed one-use alias signed after reboot", failure.message)
    }

    @Test fun fatalControlOrSigningErrorCannotBecomeExhaustion() {
        for (inControl in listOf(true, false)) {
            val failure = AssertionError("invalid control signature")
            val observed = assertFailsWith<AssertionError> {
                observe(
                    control = { if (inControl) throw failure },
                    lookup = { "key" },
                    sign = { throw failure },
                    complete = { error("fatal failure must retain marker") },
                )
            }
            assertSame(failure, observed)
        }
    }
}
