package org.hyperledger.iroha.sdk.kagemusha.candidate.lab

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Fresh physical-device process: restore, verify, redeem, reject a duplicate,
 * and export only native-observed evidence.
 */
@RunWith(AndroidJUnit4::class)
class KagemushaCandidateArtifactExportInstrumentedTest {
    @Test
    fun restartVerifyRedeemAndExportObservedEvidence() {
        CandidateLabHarness.runRestartAndExportPhase()
    }
}
