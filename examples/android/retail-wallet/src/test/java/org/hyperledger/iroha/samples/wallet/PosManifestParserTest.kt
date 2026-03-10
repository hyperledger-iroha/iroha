package org.hyperledger.iroha.samples.wallet

import java.time.Clock
import java.time.Instant
import java.time.ZoneOffset
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertTrue
import kotlin.test.fail
import org.junit.Test

class PosManifestParserTest {

    @Test
    fun `parse manifest fixture`() {
        val raw = readResource("pos_manifest.json")
        val manifest = PosManifestLoader.parse(raw)
        assertEquals("pos-retail-v1", manifest.manifestId)
        assertEquals(7, manifest.sequence)
        assertEquals(
            "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn",
            manifest.operator
        )
        assertEquals(1384, manifest.payloadBase64.length)
        assertEquals(2, manifest.backendRoots.size)
        val admission = manifest.backendRoots.first()
        assertEquals("torii-admission", admission.label)
        assertEquals("offline_admission_signer", admission.role)
        assertEquals(
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03",
            admission.publicKey
        )
    }

    @Test
    fun `invalid signature is rejected`() {
        val raw = readResource("pos_manifest.json")
        val tampered = raw.replace("D32F71FF", "FFFFFFFF")
        try {
            PosManifestLoader.parse(tampered)
        } catch (ex: IllegalArgumentException) {
            assertTrue(ex.message?.contains("manifest signature", ignoreCase = true) == true)
            return
        }
        kotlin.test.fail("Expected manifest parsing to fail when signature is tampered")
    }

    @Test
    fun `manifest status reports healthy dual signature`() {
        val manifest = PosManifestLoader.parse(readResource("pos_manifest.json"))
        val clock = Clock.fixed(Instant.ofEpochMilli(1732000000000L), ZoneOffset.UTC)
        val status = ManifestStatus.from(manifest, clock)
        assertTrue(status.dualStatusHealthy)
        assertTrue(status.warnings.isEmpty())
        assertEquals(2, status.backendRoots.size)
        assertTrue(status.backendRoots.all { it.active })
    }

    @Test
    fun `manifest status flags missing witness`() {
        val manifest = PosManifestLoader.parse(readResource("pos_manifest.json"))
        val degraded = manifest.copy(backendRoots = listOf(manifest.backendRoots.first()))
        val clock = Clock.fixed(Instant.ofEpochMilli(1732000000000L), ZoneOffset.UTC)
        val status = ManifestStatus.from(degraded, clock)
        assertFalse(status.dualStatusHealthy)
        assertTrue(status.warnings.any { it.contains("missing") })
    }

    private fun readResource(name: String): String {
        val stream = javaClass.classLoader?.getResourceAsStream(name)
        assertNotNull(stream, "missing test resource $name")
        return stream.bufferedReader().use { it.readText() }
    }
}
