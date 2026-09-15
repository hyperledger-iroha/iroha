package org.hyperledger.iroha.sdk.privacy

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

/** Exact first-release outer-suite labels without native bridge prerequisites. */
class PrivacyOuterSuiteIdsTest {
    @Test
    fun privacyStarkProtocolsSelectTheSingleSha3OuterSuite() {
        for (protocol in listOf(
            PrivacyProtocolIdV1.ZK_ACE_PQ_AUTHORIZATION_V1,
            PrivacyProtocolIdV1.IROHA_ZK_X509_STARK_P256_V1,
            PrivacyProtocolIdV1.IROHA_IVM_PRIVATE_NOTE_STARK_V1,
            PrivacyProtocolIdV1.PQ_MASP_STARK_V1,
        )) {
            assertEquals(PrivacyProofSystemIdV1.STARK_FRI_SHA3_384_GOLDILOCKS_V1, protocol.expectedProofSystem)
            assertEquals(PrivacyEngineIdV1.NATIVE_GOLDILOCKS_SHA3_384_STARK_FRI_V1, protocol.expectedEngine)
        }
    }

    @Test
    fun outerSuiteLabelsAreExactAndRejectDifferentHashes() {
        assertEquals(PrivacyProofSystemIdV1.STARK_FRI_SHA3_384_GOLDILOCKS_V1,
            PrivacyProofSystemIdV1.fromCanonicalLabel("stark-fri-sha3-384-goldilocks-v1"))
        assertEquals(PrivacyEngineIdV1.NATIVE_GOLDILOCKS_SHA3_384_STARK_FRI_V1,
            PrivacyEngineIdV1.fromCanonicalLabel("native-goldilocks-sha3-384-stark-fri-v1"))
        assertFailsWith<IllegalArgumentException> {
            PrivacyProofSystemIdV1.fromCanonicalLabel("stark-fri-poseidon-x7-goldilocks-6x64-v1")
        }
        assertFailsWith<IllegalArgumentException> {
            PrivacyEngineIdV1.fromCanonicalLabel("native-goldilocks-poseidon-x7-stark-fri-6x64-v1")
        }
    }
}
