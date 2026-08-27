package org.hyperledger.iroha.sdk.crypto.keystore

import java.io.ByteArrayInputStream
import java.security.KeyPair
import java.security.cert.CertificateFactory
import java.security.cert.X509Certificate
import java.util.Base64
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.IrohaKeyManager
import org.hyperledger.iroha.sdk.crypto.KeyManagementException
import org.hyperledger.iroha.sdk.crypto.KeyProviderMetadata
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AndroidAttestationRevocationTestFixtures
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AttestationVerificationException
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AttestationVerifier
import org.junit.jupiter.api.Test

class AttestationDispatchSecurityTest {
    @Test
    fun `manager verifies recorded challenge-bound attestation`() {
        val backend = AttestationBackend()
        val provider = KeystoreKeyProvider(backend, KeyGenParameters.builder().build())
        val manager = IrohaKeyManager.fromProviders(listOf(provider))
        val challenge = FIXTURE_CHALLENGE.copyOf()

        val result = manager.verifyAttestation("wallet", verifier(), challenge)

        assertNotNull(result)
        assertTrue(result.isStrongBoxAttestation)
        assertContentEquals(FIXTURE_CHALLENGE, result.attestationChallenge())
        assertContentEquals(FIXTURE_CHALLENGE, challenge)
        assertEquals(0, backend.generationCalls)
        assertEquals(1, backend.readCalls)
    }

    @Test
    fun `challenge verification bypasses cached attestation material`() {
        val backend = AttestationBackend()
        val provider = KeystoreKeyProvider(backend, KeyGenParameters.builder().build())
        val verifier = verifier()

        assertNotNull(provider.verifyAttestation("wallet", verifier, FIXTURE_CHALLENGE))
        assertNotNull(provider.verifyAttestation("wallet", verifier, FIXTURE_CHALLENGE))

        assertEquals(0, backend.generationCalls)
        assertEquals(2, backend.readCalls)
    }

    @Test
    fun `challenge verification propagates recorded attestation read failures`() {
        val backend = AttestationBackend(attestationReadFails = true)
        val provider = KeystoreKeyProvider(backend, KeyGenParameters.builder().build())

        assertFailsWith<AttestationVerificationException> {
            provider.verifyAttestation("wallet", verifier(), FIXTURE_CHALLENGE)
        }

        assertEquals(0, backend.generationCalls)
        assertEquals(1, backend.readCalls)
    }

    @Test
    fun `manager rejects one alias resolving to different provider keys`() {
        val matching = KeystoreKeyProvider(
            AttestationBackend(AliasLoadMode.MATCHING),
            KeyGenParameters.builder().build(),
        )
        val conflicting = KeystoreKeyProvider(
            AttestationBackend(AliasLoadMode.MISMATCHED),
            KeyGenParameters.builder().build(),
        )
        val manager = IrohaKeyManager.fromProviders(listOf(matching, conflicting))

        val error = assertFailsWith<AttestationVerificationException> {
            manager.verifyAttestation("wallet", verifier(), FIXTURE_CHALLENGE)
        }
        assertTrue(error.message.orEmpty().contains("different public keys"))
    }

    @Test
    fun `manager rejects absent failed and mismatched alias keys`() {
        val verifier = verifier()
        for (mode in listOf(
            AliasLoadMode.MISSING,
            AliasLoadMode.FAILING,
            AliasLoadMode.MISMATCHED,
        )) {
            val directBackend = AttestationBackend(mode)
            val directProvider = KeystoreKeyProvider(
                directBackend,
                KeyGenParameters.builder().build(),
            )
            assertFailsWith<AttestationVerificationException>("direct ${mode.name}") {
                directProvider.verifyAttestation("wallet", verifier, FIXTURE_CHALLENGE)
            }

            val managerBackend = AttestationBackend(mode)
            val manager = IrohaKeyManager.fromProviders(
                listOf(KeystoreKeyProvider(managerBackend, KeyGenParameters.builder().build()))
            )

            assertFailsWith<AttestationVerificationException>(mode.name) {
                manager.verifyAttestation("wallet", verifier, FIXTURE_CHALLENGE)
            }
        }
    }

    @Test
    fun `provider and manager reject absent challenges before backend dispatch`() {
        val backend = AttestationBackend()
        val provider = KeystoreKeyProvider(backend, KeyGenParameters.builder().build())
        val manager = IrohaKeyManager.fromProviders(listOf(provider))
        val verifier = verifier()

        assertFailsWith<AttestationVerificationException> {
            provider.verifyAttestation("wallet", verifier)
        }
        assertFailsWith<AttestationVerificationException> {
            provider.verifyAttestation("wallet", verifier, null)
        }
        assertFailsWith<AttestationVerificationException> {
            provider.verifyAttestation("wallet", verifier, ByteArray(0))
        }
        assertFailsWith<AttestationVerificationException> {
            manager.verifyAttestation("wallet", verifier)
        }
        assertFailsWith<AttestationVerificationException> {
            manager.verifyAttestation("wallet", verifier, null)
        }
        assertFailsWith<AttestationVerificationException> {
            manager.verifyAttestation("wallet", verifier, ByteArray(0))
        }

        assertEquals(0, backend.generationCalls)
        assertEquals(0, backend.readCalls)
    }

    private fun verifier(): AttestationVerifier {
        val policy = AndroidAttestationRevocationTestFixtures.policy(
            EVALUATION_TIME_EPOCH_MILLIS,
            86_400L,
        )
        return AttestationVerifier.builder(policy, EVALUATION_TIME_EPOCH_MILLIS)
            .addTrustedRoot(ROOT_CERT)
            .requireStrongBox(true)
            .build()
    }

    private enum class AliasLoadMode {
        MATCHING,
        MISSING,
        MISMATCHED,
        FAILING,
    }

    private class AttestationBackend(
        private val aliasLoadMode: AliasLoadMode = AliasLoadMode.MATCHING,
        private val attestationReadFails: Boolean = false,
    ) : KeystoreBackend {
        var generationCalls = 0
        var readCalls = 0
        var observedChallenge = ByteArray(0)

        override fun load(alias: String): KeyPair? = when (aliasLoadMode) {
            AliasLoadMode.MATCHING -> KeyPair(decodeCertificate(STRONGBOX_CERT).publicKey, null)
            AliasLoadMode.MISSING -> null
            AliasLoadMode.MISMATCHED -> KeyPair(decodeCertificate(ROOT_CERT).publicKey, null)
            AliasLoadMode.FAILING -> throw KeyManagementException("fixture load failure")
        }

        override fun generate(
            alias: String,
            parameters: KeyGenParameters,
        ): KeyGenerationResult = throw KeyManagementException("not used")

        override fun generateEphemeral(parameters: KeyGenParameters): KeyPair =
            throw KeyManagementException("not used")

        override fun metadata(): KeyProviderMetadata =
            KeyProviderMetadata.strongBox("fixture-keystore", true)

        override fun name(): String = "fixture-keystore"

        override fun attestation(alias: String): KeyAttestation? {
            readCalls++
            if (attestationReadFails) {
                throw KeyManagementException("fixture attestation read failure")
            }
            return fixtureAttestation(alias)
        }

        override fun generateAttestation(alias: String, challenge: ByteArray): KeyAttestation? {
            generationCalls++
            observedChallenge = challenge.copyOf()
            challenge.fill(0)
            return fixtureAttestation(alias)
        }

        private fun fixtureAttestation(alias: String): KeyAttestation =
            KeyAttestation.builder()
                .setAlias(alias)
                .addCertificate(STRONGBOX_CERT)
                .addCertificate(ROOT_CERT)
                .build()
    }

    companion object {
        private const val EVALUATION_TIME_EPOCH_MILLIS = 1_761_408_000_000L
        private val FIXTURE_CHALLENGE = hex("4145454245")
        private val ROOT_CERT = decodeBase64(
            "MIIDIjCCAgqgAwIBAgIUHifREEUziVTjk5SY9EdEKBhj+LAwDQYJKoZIhvcNAQELBQAwFzEVMBMGA1UE" +
                "AwwMVGVzdCBSb290IENBMB4XDTI1MTAyNTE1Mjc0M1oXDTM1MTAyMzE1Mjc0M1owFzEVMBMGA1UEAwwM" +
                "VGVzdCBSb290IENBMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA4cr8VyFyforGk8BkefC2" +
                "jy36UydWa50h/9tCGhx+JeYpsmNE050wPQTZJ+09vTjZN9N2dO/Bh8TGd4nIW5D+swmXrsnzyt9fpMMR" +
                "PrDpmXTAvaDdD+afCgTRkEasSb7wGNh7wtgUvP5aQnTRFHEPN8VVn31ndv093Ex84PvKgQt3SYQuW+ho" +
                "zw1TZyAhjc4ydGTX3szxx1SJNtnxCBWAspaCKVXo4vCgSHUO6/JXW8BfaCckAniGqrNySk35POmmlw70" +
                "oj0zuoqoeWygwZVnGXMAvkN6gVmW/OY18cvAhZHlLfJG0P/o+i7DTpllebDM6W7ILF+YTxEXrfi2ixdw" +
                "QwIDAQABo2YwZDAdBgNVHQ4EFgQUAiNcsp2ChOMGPTVGbslvK4wPnVQwHwYDVR0jBBgwFoAUAiNcsp2C" +
                "hOMGPTVGbslvK4wPnVQwEgYDVR0TAQH/BAgwBgEB/wIBATAOBgNVHQ8BAf8EBAMCAYYwDQYJKoZIhvcN" +
                "AQELBQADggEBAH1/kr4JUjckOxPIR0XdZE73Wwr4DXqCb/InpBs+2TJJPnXONpuwNtLPtFUyV9FuJ9qM" +
                "H+M2aGu3+enncDnaw8ChAPKn9+QmjgTrZk9sPQV9zi6coIrMqD67gMwJW7HE0YDem7pNpiN1l/VvDrwe" +
                "V/2QJu7Og+rDvVc48TIhVeTEaQLURsgwi2R8U/usieuDysfPq7OJm/1eu8pE+etK5GiR9t/24qfx8V8d" +
                "DVliRz7PjoxZoDZrgpJl94nq5665BpXQ5lbsrr22EFgqxkMs1nPNIUFVxgEZUPnOzPVGPEOefnSjuKxT" +
                "AR7INRwTwVOtoGf0swuwJo3VZHgfAcaLfLM=",
        )
        private val STRONGBOX_CERT = decodeBase64(
            "MIIDUTCCAjmgAwIBAgIULKS+BqcxAYB6ooMNchJ4LI59fxowDQYJKoZIhvcNAQELBQAwFzEVMBMGA1UE" +
                "AwwMVGVzdCBSb290IENBMB4XDTI1MTAyNTE1MjgwMFoXDTI2MTAyNTE1MjgwMFowHzEdMBsGA1UEAwwU" +
                "VGVzdCBBdHRlc3RhdGlvbiBLZXkwggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQCbQVFuKFDD" +
                "6t52BMS3ZVot+5OPrSIcXlY1xRgXJoh+yhmXjfc5UIBgjWyNuLWyaT8N6+iVUNqLsh7Nbow8ySi1vgWI" +
                "56OVhc4yLf6z2kbwTqJScHwQbphed/wLA3I0tkzu1E0zt3AqNsPlEEMiZYHe3PBbvLBrx+Ug+UsPe0uZ" +
                "UxU5l6fDd9MeWihEvnOCWX1Fi9D4IfOeNq1UiZlkzih97JhqEWx32FVyxOdM2gx/VySv6R4KGu3nVRzA" +
                "cl4Lgw2Zex81/x9TKu5Mnf+Sz+sYtPLfS+D7R5xHI/GZPZ/SHZ8g79dm0o6D/5S1B29kolGMAnnbLN3H" +
                "ym7WJm9tVf3zAgMBAAGjgYwwgYkwHQYDVR0OBBYEFAL9ObHQIHwRJ2kTOTzbnwzAM9o0MB8GA1UdIwQY" +
                "MBaAFAIjXLKdgoTjBj01Rm7JbyuMD51UMAkGA1UdEwQCMAAwCwYDVR0PBAQDAgeAMC8GCisGAQQB1nkC" +
                "AREEITAfAgEDCgECAgEECgECBAVBRUVCRQQEAQIDBDAAMAAwADANBgkqhkiG9w0BAQsFAAOCAQEAE+vf" +
                "oKnq0xblVQmxeT8IjRRqzFnIpa7Fd92xoGSydhNwV1Ox29rPOkOthq3om/r03rETj07LbArH8iyfCs5m" +
                "cSrfWC+kELgKuWEVYs7Zi20UanZsV7lnYXaqTKt8uPLh4TDRbZ6ymRi5ionLJ8vu8cfEyAVCKmn983Kr" +
                "bMgwIYzmWPMPnp+oCJ/TXOLQjTgbmcP3QmXPs7BBjdasixlvmBForI08Y5qClDZMOqBf/l5xQi4IeLr9" +
                "Q3mFG3KuAmuoZKvKN6TAvY5Hleqy9pg4gKSB7/0wK5lfX/JfkLi6erS5l8VuED6OcOZc3VbO8OrwRdlP" +
                "FxdGTgtauVtYo24deQ==",
        )

        private fun decodeBase64(value: String): ByteArray = Base64.getDecoder().decode(value)

        private fun decodeCertificate(value: ByteArray): X509Certificate =
            CertificateFactory.getInstance("X.509")
                .generateCertificate(ByteArrayInputStream(value)) as X509Certificate

        private fun hex(value: String): ByteArray =
            ByteArray(value.length / 2) { index ->
                value.substring(index * 2, index * 2 + 2).toInt(16).toByte()
            }
    }
}
