package org.hyperledger.iroha.sdk.offline

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class KagemushaRecursiveSpendProverTest {
    @Test
    fun exactAbi19IsRequired() {
        assertTrue(KagemushaRecursiveSpendProver.isExactBridgeAbi(19))
        assertFalse(KagemushaRecursiveSpendProver.isExactBridgeAbi(20))
        assertTrue(
            KagemushaRecursiveSpendProver.detectExactNativeAvailability(
                loadLibrary = {},
                abiVersion = { 19 },
                symbolProbe = { true },
            ),
        )
    }

    @Test
    fun artifactContractAndInventoryAreCurrentOnly() {
        assertEquals(19, KagemushaRecursiveSpendProver.REQUIRED_NATIVE_BRIDGE_ABI_VERSION)
        assertEquals(6, KagemushaRecursiveSpendProver.ARTIFACT_COUNT)
        assertEquals(
            "kagemusha.offline.recursive_spend.artifact_manifest.v3",
            KagemushaRecursiveSpendProver.ARTIFACT_MANIFEST_SCHEMA,
        )
        assertEquals(
            listOf(
                "transition-eq.parameters.krv3",
                "transition-eq.proving-key.krv3",
                "transition-eq.verifying-key.krv3",
                "state-ep.parameters.krv3",
                "state-ep.proving-key.krv3",
                "state-ep.verifying-key.krv3",
            ),
            KagemushaRecursiveSpendProver.ARTIFACT_FILES,
        )
        val methods = KagemushaRecursiveSpendProver::class.java.declaredMethods
            .filter {
                java.lang.reflect.Modifier.isPublic(it.modifiers) &&
                    !it.isSynthetic &&
                    !it.name.startsWith("access\$")
            }
            .map { it.name }
            .toSet()
        assertEquals(
            setOf(
                "beginArtifactIngest",
                "beginArtifactInstallSession",
                "isArtifactStreamingAvailable",
                "isProofBackendAvailable",
            ),
            methods,
        )
    }
}
