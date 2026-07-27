package org.hyperledger.iroha.sdk.privacy

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotSame

class PrivacyNativeBridgeTest {
    private val expected =
        listOf(
            "zk-ace-pq-authorization-v0",
            "anonymous-pgc-k-out-of-n-v1",
            "verange-transparent-range-v1",
            "iroha-zk-ams-v1",
            "vega-existing-credential-zk-v0",
            "iroha-zk-x509-stark-p256-v0",
            "iroha-jindo-polynomial-commitment-v0",
            "iroha-bootle-lantern-anoncred-v1",
            "orchard-halo2-actions-v1",
            "monero-fcmp-plus-plus-v1",
            "iroha-ivm-private-note-stark-v1",
            "pq-masp-stark-v0",
        )

    @Test
    fun exactClosedRegistryIsStable() {
        assertEquals(21, PrivacyNativeBridge.REQUIRED_BRIDGE_ABI_VERSION)
        assertEquals(expected, PrivacyNativeBridge.protocolsV1().map { it.canonicalLabel })
        assertEquals(12, PrivacyNativeBridge.protocolsV1().size)
        expected.forEachIndexed { index, label ->
            assertEquals(
                PrivacyNativeBridge.protocolsV1()[index],
                PrivacyNativeBridge.ProtocolIdV1.fromCanonicalLabel(label),
            )
        }
    }

    @Test
    fun aliasesAndNonCanonicalSpellingsAreRejected() {
        listOf(
            "jindo-lattice-pcs-zk-v0",
            "sis-hints-anoncred-pq-v0",
            "silent-threshold-anoncred-v0",
            "zk-ams-recursive-admission-v0",
            "iroha-zk-ams-v1 ",
            "Iroha-Zk-Ams-V1",
            "",
            "unknown-privacy-protocol-v1",
        ).forEach { rejected ->
            assertFailsWith<IllegalArgumentException> {
                PrivacyNativeBridge.ProtocolIdV1.fromCanonicalLabel(rejected)
            }
        }
    }

    @Test
    fun capabilityArchiveValidationFailsClosed() {
        assertFailsWith<IllegalStateException> {
            PrivacyNativeBridge.requireCapabilityArchive(null)
        }
        assertFailsWith<IllegalStateException> {
            PrivacyNativeBridge.requireCapabilityArchive(ByteArray(39))
        }
        val badMagic = capabilityArchive().also { it[0] = 'X'.code.toByte() }
        assertFailsWith<IllegalStateException> {
            PrivacyNativeBridge.requireCapabilityArchive(badMagic)
        }
        val badSchema = capabilityArchive().also { it[13] = 0x51 }
        assertFailsWith<IllegalStateException> {
            PrivacyNativeBridge.requireCapabilityArchive(badSchema)
        }
    }

    @Test
    fun capabilityArchiveReturnsDefensiveCopy() {
        val archive = capabilityArchive()
        val accepted = PrivacyNativeBridge.requireCapabilityArchive(archive)
        assertNotSame(archive, accepted)
        assertContentEquals(archive, accepted)
        archive[0] = 'X'.code.toByte()
        assertEquals('N'.code.toByte(), accepted[0])
    }

    @Test
    fun retiredGenericProofSurfaceIsAbsent() {
        PrivacyNativeBridge::class.java.declaredMethods.forEach { method ->
            val name = method.name.lowercase()
            check(!name.contains("proofrequest")) { method.name }
            check(!name.contains("buildproof")) { method.name }
            check(!name.contains("verifyproof")) { method.name }
        }
    }

    private fun capabilityArchive(): ByteArray =
        ByteArray(40).also {
            it[0] = 'N'.code.toByte()
            it[1] = 'R'.code.toByte()
            it[2] = 'T'.code.toByte()
            it[3] = '0'.code.toByte()
            it.fill(0x50.toByte(), 6, 22)
        }
}
