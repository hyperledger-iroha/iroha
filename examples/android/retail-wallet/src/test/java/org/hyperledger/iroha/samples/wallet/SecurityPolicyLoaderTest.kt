package org.hyperledger.iroha.samples.wallet

import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import org.junit.Test

class SecurityPolicyLoaderTest {

    @Test
    fun `parse policy honors overrides`() {
        val json = """
            {
              "policy_version": "demo",
              "enforced_certificate_id_hex": "ABCDEF",
              "expected_attestation_nonce_hex": "0102",
              "verdict_grace_period_ms": 1000,
              "pinned_roots": [
                {"alias": "demo-root", "sha256": "00"}
              ]
            }
        """.trimIndent()
        val policy = SecurityPolicyLoader.parse(
            json,
            SecurityPolicyOverrides(graceOverrideMs = 5000L, graceProfile = null)
        )
        assertEquals("demo", policy.version)
        assertEquals("abcdef", policy.enforcedCertificateIdHex)
        assertEquals("0102", policy.expectedAttestationNonceHex)
        assertEquals(5000L, policy.verdictGracePeriodMs)
        assertEquals(SecurityPolicy.GracePeriodSource.OVERRIDE, policy.gracePeriodSource)
        assertEquals(null, policy.gracePeriodProfile)
        assertEquals(1, policy.pinnedRoots.size)
        assertEquals("demo-root", policy.pinnedRoots.first().alias)
        assertEquals("00", policy.pinnedRoots.first().sha256)
    }

    @Test
    fun `parse policy selects profile when override missing`() {
        val json = """
            {
              "policy_version": "demo",
              "enforced_certificate_id_hex": "ABCDEF",
              "expected_attestation_nonce_hex": "0102",
              "verdict_grace_period_ms": 1000,
              "grace_period_profiles": {
                "short": 60000,
                "extended": 7200000
              },
              "pinned_roots": []
            }
        """.trimIndent()
        val policy = SecurityPolicyLoader.parse(
            json,
            SecurityPolicyOverrides(graceOverrideMs = null, graceProfile = "SHORT")
        )
        assertEquals(60000L, policy.verdictGracePeriodMs)
        assertEquals(SecurityPolicy.GracePeriodSource.PROFILE, policy.gracePeriodSource)
        assertEquals("short", policy.gracePeriodProfile?.lowercase())
        assertEquals(1000L, policy.defaultGracePeriodMs)
        assertEquals(
            mapOf("short" to 60000L, "extended" to 7200000L),
            policy.availableGraceProfiles
        )
    }

    @Test
    fun `fingerprint matches expected digest`() {
        val resource = javaClass.getResourceAsStream("/mock_root.pem")
        assertNotNull(resource)
        resource.use {
            val fingerprint = PinnedCertificateVerifier.fingerprint(it)
            assertEquals(
                "bf25fa06eb409c2e022b263cfeb7fe168a9491f506fa78ab3fc7202f2842cf8b",
                fingerprint
            )
        }
    }
}
