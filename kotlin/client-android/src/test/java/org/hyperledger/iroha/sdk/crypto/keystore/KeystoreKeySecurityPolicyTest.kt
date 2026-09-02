package org.hyperledger.iroha.sdk.crypto.keystore

import java.security.KeyPair
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertFailsWith
import kotlin.test.assertSame
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test
import org.hyperledger.iroha.sdk.IrohaKeyManager
import org.hyperledger.iroha.sdk.crypto.KeyGenerationOutcome
import org.hyperledger.iroha.sdk.crypto.KeyManagementException
import org.hyperledger.iroha.sdk.crypto.KeyProvider
import org.hyperledger.iroha.sdk.crypto.KeyProviderMetadata
import org.hyperledger.iroha.sdk.crypto.SigningAlgorithm
import org.hyperledger.iroha.sdk.crypto.SoftwareKeyProvider
import org.hyperledger.iroha.sdk.telemetry.KeystoreTelemetryEmitter

class KeystoreKeySecurityPolicyTest {
    @Test
    fun `default providers always include a software signing path`() {
        val parameters = KeyGenParameters.builder()
            .setSigningAlgorithm(SigningAlgorithm.ML_DSA)
            .build()
        for (manager in listOf(
            IrohaKeyManager.withDefaultProviders(parameters),
            IrohaKeyManager.withDefaultProviders(
                parameters,
                KeystoreTelemetryEmitter.noop(),
            ),
        )) {
            assertEquals(
                listOf(KeyProviderMetadata.HardwareSecurityLevel.NONE),
                manager.providerMetadata().map { it.securityLevel },
            )
        }
    }

    @Test
    fun `required hardware policies reject a weaker existing alias`() {
        val existing = SoftwareKeyProvider().generateEphemeral()
        val backend = MixedSecurityBackend(
            capability = KeyProviderMetadata.strongBox("mixed-keystore", false),
            keySecurity = KeyProviderMetadata.software("mixed-keystore"),
            existing = existing,
        )
        val manager = manager(backend)

        assertFailsWith<KeyManagementException> {
            manager.generateOrLoad("account", KeySecurityPreference.HARDWARE_REQUIRED)
        }
        assertFailsWith<KeyManagementException> {
            manager.generateOrLoad("account", KeySecurityPreference.STRONGBOX_REQUIRED)
        }
    }

    @Test
    fun `generated route uses measured key metadata instead of request flag`() {
        val backend = MixedSecurityBackend(
            capability = KeyProviderMetadata.strongBox("mixed-keystore", false),
            keySecurity = KeyProviderMetadata.software("mixed-keystore"),
            reportGeneratedStrongBox = true,
        )
        val provider = provider(backend)

        assertFailsWith<KeyManagementException> {
            provider.generateWithOutcome("strict", KeySecurityPreference.STRONGBOX_REQUIRED)
        }
        val preferred = provider.generateWithOutcome(
            "preferred",
            KeySecurityPreference.STRONGBOX_PREFERRED,
        )
        assertEquals(KeyGenerationOutcome.Route.SOFTWARE, preferred.route)
    }

    @Test
    fun `StrongBox preferred retries the keystore without StrongBox`() {
        val backend = StrongBoxRequestFailingBackend(strongBoxUnavailable = true)

        val outcome = provider(backend).generateWithOutcome(
            "preferred-keystore-fallback",
            KeySecurityPreference.STRONGBOX_PREFERRED,
        )

        assertEquals(KeyGenerationOutcome.Route.SOFTWARE, outcome.route)
        assertEquals(2, backend.requests.size)
        assertTrue(backend.requests[0].preferStrongBox)
        assertFalse(backend.requests[0].requireStrongBox)
        assertFalse(backend.requests[1].preferStrongBox)
        assertFalse(backend.requests[1].requireStrongBox)
    }

    @Test
    fun `direct Android backend policy retries preferred generation without StrongBox`() {
        val requests = mutableListOf<KeyGenParameters>()
        val expected = SoftwareKeyProvider().generateEphemeral()
        val parameters = KeyGenParameters.Builder()
            .setPreferStrongBox(true)
            .build()

        val result = generateAndroidKeystoreWithPreferredStrongBoxFallback(parameters) { request ->
            requests += request
            if (request.preferStrongBox) {
                throw KeyManagementException(
                    "fixture key generation failure",
                    StrongBoxUnavailableFailure("fixture StrongBox unavailable"),
                )
            }
            KeyGenerationResult(expected, false)
        }

        assertSame(expected, result.keyPair)
        assertEquals(2, requests.size)
        assertTrue(requests[0].preferStrongBox)
        assertFalse(requests[1].preferStrongBox)
        assertFalse(requests[1].requireStrongBox)
    }

    @Test
    fun `direct Android backend policy rejects a weaker required result`() {
        val parameters = KeyGenParameters.Builder()
            .setRequireStrongBox(true)
            .setPreferStrongBox(true)
            .build()
        var attempts = 0

        assertFailsWith<KeyManagementException> {
            generateAndroidKeystoreWithPreferredStrongBoxFallback(parameters) {
                attempts += 1
                KeyGenerationResult(SoftwareKeyProvider().generateEphemeral(), false)
            }
        }

        assertEquals(1, attempts)
    }

    @Test
    fun `preferred StrongBox parameters retry plain generation without StrongBox`() {
        val backend = StrongBoxRequestFailingBackend(strongBoxUnavailable = true)
        val parameters = KeyGenParameters.Builder()
            .setPreferStrongBox(true)
            .build()

        KeystoreKeyProvider(backend, parameters).generate("preferred-parameters-fallback")

        assertEquals(2, backend.requests.size)
        assertTrue(backend.requests[0].preferStrongBox)
        assertFalse(backend.requests[1].preferStrongBox)
        assertFalse(backend.requests[1].requireStrongBox)
    }

    @Test
    fun `StrongBox preferred does not downgrade an unrelated generation failure`() {
        val backend = StrongBoxRequestFailingBackend(strongBoxUnavailable = false)

        assertFailsWith<KeyManagementException> {
            provider(backend).generateWithOutcome(
                "preferred-unrelated-failure",
                KeySecurityPreference.STRONGBOX_PREFERRED,
            )
        }
        assertEquals(1, backend.requests.size)
    }

    @Test
    fun `StrongBox required rejects generation failure without fallback`() {
        val backend = StrongBoxRequestFailingBackend(strongBoxUnavailable = true)
        val manager = IrohaKeyManager.fromProviders(
            listOf(provider(backend), SoftwareKeyProvider()),
        )

        assertFailsWith<KeyManagementException> {
            manager.generateOrLoad(
                "required-no-fallback",
                KeySecurityPreference.STRONGBOX_REQUIRED,
            )
        }
        assertEquals(1, backend.requests.size)
        assertTrue(backend.requests.single().requireStrongBox)
        assertTrue(backend.requests.single().preferStrongBox)
    }

    @Test
    fun `verified StrongBox existing alias remains accepted`() {
        val existing = SoftwareKeyProvider().generateEphemeral()
        val strongBox = KeyProviderMetadata.strongBox("mixed-keystore", false)
        val backend = MixedSecurityBackend(
            capability = strongBox,
            keySecurity = strongBox,
            existing = existing,
        )

        val loaded = manager(backend).generateOrLoad(
            "account",
            KeySecurityPreference.STRONGBOX_REQUIRED,
        )

        assertSame(existing, loaded)
    }

    @Test
    fun `generic provider capability is not per-key proof`() {
        val keyPair = SoftwareKeyProvider().generateEphemeral()
        val capability = KeyProviderMetadata.strongBox("capability-only", false)

        for (existing in listOf(keyPair, null)) {
            val manager = IrohaKeyManager.fromProviders(
                listOf(CapabilityOnlyProvider(capability, existing))
            )
            assertFailsWith<KeyManagementException> {
                manager.generateOrLoad(
                    "capability-only",
                    KeySecurityPreference.STRONGBOX_REQUIRED,
                )
            }
        }
    }

    @Test
    fun `withPreference retains hardware required for plain generation`() {
        val backend = MixedSecurityBackend(
            capability = KeyProviderMetadata.strongBox("mixed-keystore", false),
            keySecurity = KeyProviderMetadata.software("mixed-keystore"),
        )
        val provider = provider(backend).withPreference(KeySecurityPreference.HARDWARE_REQUIRED)

        assertFailsWith<KeyManagementException> {
            provider.generate("strict-default")
        }
    }

    private fun manager(backend: KeystoreBackend): IrohaKeyManager =
        IrohaKeyManager.fromProviders(listOf(provider(backend)))

    private fun provider(backend: KeystoreBackend): KeystoreKeyProvider =
        KeystoreKeyProvider(backend, KeyGenParameters.builder().build())

    private class MixedSecurityBackend(
        private val capability: KeyProviderMetadata,
        private val keySecurity: KeyProviderMetadata,
        private val existing: KeyPair? = null,
        private val reportGeneratedStrongBox: Boolean = false,
    ) : KeystoreBackend {
        private val software = SoftwareKeyProvider()

        override fun load(alias: String): KeyPair? = existing

        override fun generate(
            alias: String,
            parameters: KeyGenParameters,
        ): KeyGenerationResult =
            KeyGenerationResult(software.generateEphemeral(), reportGeneratedStrongBox)

        override fun generateEphemeral(parameters: KeyGenParameters): KeyPair =
            software.generateEphemeral()

        override fun metadata(): KeyProviderMetadata = capability

        override fun keyMetadata(alias: String, keyPair: KeyPair): KeyProviderMetadata =
            keySecurity

        override fun name(): String = capability.name
    }

    private class StrongBoxRequestFailingBackend(
        private val strongBoxUnavailable: Boolean,
    ) : KeystoreBackend {
        private val software = SoftwareKeyProvider()
        val requests = mutableListOf<KeyGenParameters>()

        override fun load(alias: String): KeyPair? = null

        override fun generate(
            alias: String,
            parameters: KeyGenParameters,
        ): KeyGenerationResult {
            requests += parameters
            if (parameters.requireStrongBox || parameters.preferStrongBox) {
                val cause = if (strongBoxUnavailable) {
                    StrongBoxUnavailableFailure("fixture StrongBox unavailable")
                } else {
                    IllegalStateException("fixture unrelated keystore failure")
                }
                throw KeyManagementException("fixture key generation failure", cause)
            }
            return KeyGenerationResult(software.generateEphemeral(), false)
        }

        override fun generateEphemeral(parameters: KeyGenParameters): KeyPair =
            software.generateEphemeral()

        override fun metadata(): KeyProviderMetadata =
            KeyProviderMetadata.strongBox("failing-strongbox-keystore", false)

        override fun keyMetadata(alias: String, keyPair: KeyPair): KeyProviderMetadata =
            KeyProviderMetadata.software("failing-strongbox-keystore")

        override fun name(): String = "failing-strongbox-keystore"
    }

    private class CapabilityOnlyProvider(
        private val capability: KeyProviderMetadata,
        private val existing: KeyPair?,
    ) : KeyProvider {
        private val software = SoftwareKeyProvider()

        override fun load(alias: String): KeyPair? = existing

        override fun generate(alias: String): KeyPair = software.generateEphemeral()

        override fun generateEphemeral(): KeyPair = software.generateEphemeral()

        override fun isHardwareBacked(): Boolean = capability.hardwareBacked

        override fun name(): String = capability.name

        override fun metadata(): KeyProviderMetadata = capability
    }
}
