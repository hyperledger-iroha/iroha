package org.hyperledger.iroha.sdk.offline

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertSame
import kotlin.test.assertTrue

class KagemushaRecursiveSpendProverTest {
    @Test
    fun `exact ABI and single mode fail closed`() {
        assertEquals(18, KagemushaRecursiveSpendProver.REQUIRED_NATIVE_BRIDGE_ABI_VERSION)
        assertTrue(KagemushaRecursiveSpendProver.isExactBridgeAbi(18))
        assertFalse(KagemushaRecursiveSpendProver.isExactBridgeAbi(17))
        assertFalse(KagemushaRecursiveSpendProver.isExactBridgeAbi(19))
        assertEquals(listOf(KagemushaRecursiveSpendProver.Mode.RECURSIVE_SPEND),
            KagemushaRecursiveSpendProver.Mode.entries)
        assertEquals("recursive_spend_v2", KagemushaRecursiveSpendProver.MODE)
        assertEquals(
            "recursive_spend_v2",
            KagemushaRecursiveSpendProver.Mode.RECURSIVE_SPEND.wireName,
        )
        assertNull(KagemushaRecursiveSpendProver.preferredMode(false))
        assertSame(
            KagemushaRecursiveSpendProver.Mode.RECURSIVE_SPEND,
            KagemushaRecursiveSpendProver.preferredMode(true),
        )
    }

    @Test
    fun `malformed artifact inputs fail before native dispatch`() {
        val digest = ByteArray(32) { 1 }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.beginArtifactInstallSession(null, digest)
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.beginArtifactInstallSession(ByteArray(0), digest)
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.beginArtifactInstallSession(
                ByteArray(KagemushaRecursiveSpendProver.MAX_MANIFEST_BYTES + 1),
                digest,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.beginArtifactInstallSession(byteArrayOf(1), null)
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.beginArtifactInstallSession(byteArrayOf(1), ByteArray(31))
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.beginArtifactInstallSession(byteArrayOf(1), ByteArray(32))
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.beginArtifactIngest(
                byteArrayOf(1),
                digest,
                ByteArray(32),
            )
        }
    }

    @Test
    fun `install session rejects partial and closed use`() {
        val session = KagemushaRecursiveSpendProver.ArtifactInstallSession(
            byteArrayOf(1),
            ByteArray(32) { 1 },
        )
        assertFailsWith<IllegalStateException> { session.install() }
        session.close()
        assertFalse(session.isInstalled())
        assertFailsWith<IllegalStateException> {
            session.beginArtifact(ByteArray(32) { 2 })
        }
    }
}
