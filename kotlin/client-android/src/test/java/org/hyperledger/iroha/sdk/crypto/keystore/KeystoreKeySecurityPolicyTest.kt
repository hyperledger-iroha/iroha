package org.hyperledger.iroha.sdk.crypto.keystore

import java.security.KeyPair
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertSame
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
