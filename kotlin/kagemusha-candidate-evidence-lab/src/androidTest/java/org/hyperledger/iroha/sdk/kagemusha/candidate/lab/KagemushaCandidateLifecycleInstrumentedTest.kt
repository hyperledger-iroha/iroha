package org.hyperledger.iroha.sdk.kagemusha.candidate.lab

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Test
import org.junit.runner.RunWith

/** First physical-device process: install the candidate, initialize, and prove two hops. */
@RunWith(AndroidJUnit4::class)
class KagemushaCandidateLifecycleInstrumentedTest {
    @Test
    fun proveAndPersistObservedMultiHopCheckpoint() {
        CandidateLabHarness.runProofPhase()
    }
}
