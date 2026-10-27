// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

/** Tests bookkeeping used by both real probe owners; Native admission remains independent. */
class KagemushaProbeReviewPhaseTest {
    @Test fun acknowledgedFenceAndTypedRetentionPermitOnlyExplicitRetry() {
        val phase = KagemushaProbeReviewPhase(); var publications = 0; var exactChecks = 0
        assertThrows(IllegalStateException::class.java) { phase.retry { exactChecks++ } }
        phase.beginConfirmation(); phase.publishFence { publications++ }; phase.retained()
        assertEquals(KagemushaProbeReviewPhase.State.APPROVED_LIVE_RETAINED, phase.state)
        assertThrows(IllegalStateException::class.java) { phase.beginConfirmation() }
        assertThrows(IllegalStateException::class.java) { phase.publishFence { publications++ } }
        phase.retry { exactChecks++ }; phase.retained(); phase.retry { exactChecks++ }; phase.finish()
        assertEquals(1, publications); assertEquals(2, exactChecks)
        assertThrows(IllegalStateException::class.java) { phase.retry { exactChecks++ } }
        assertEquals(2, exactChecks)
    }
    @Test fun publicationOrExactFenceCheckFailureCannotRestoreApproval() {
        val phase = KagemushaProbeReviewPhase(); phase.beginConfirmation()
        val failure = IllegalStateException("lost publication acknowledgment")
        assertSame(failure, assertThrows(IllegalStateException::class.java) { phase.publishFence { throw failure } })
        assertEquals(KagemushaProbeReviewPhase.State.FINISHED, phase.state)
        assertThrows(IllegalStateException::class.java) { phase.retained() }
        assertThrows(IllegalStateException::class.java) { phase.retry {} }
        val retained = KagemushaProbeReviewPhase(); retained.beginConfirmation(); retained.publishFence {}; retained.retained()
        assertThrows(IllegalStateException::class.java) { retained.retry { throw failure } }
        assertEquals(KagemushaProbeReviewPhase.State.FINISHED, retained.state)
        assertThrows(IllegalStateException::class.java) { retained.retry {} }
    }
    @Test fun authenticationFailureAllowsNewDecisionButEndDestroysLiveRetry() {
        val phase = KagemushaProbeReviewPhase(); phase.beginConfirmation(); phase.confirmationFailed()
        assertEquals(KagemushaProbeReviewPhase.State.AWAITING_CONFIRMATION, phase.state)
        phase.beginConfirmation(); phase.publishFence {}; phase.retained(); phase.endRequested()
        assertEquals(KagemushaProbeReviewPhase.State.FINISHED, phase.state)
        assertThrows(IllegalStateException::class.java) { phase.retry {} }
        // New actor state never reconstructs an approved capability from a root file.
        val reopened = KagemushaProbeReviewPhase()
        assertThrows(IllegalStateException::class.java) { reopened.retry {} }
    }
}
