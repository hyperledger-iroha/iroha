package org.hyperledger.iroha.sdk

import java.security.KeyPair
import org.hyperledger.iroha.sdk.crypto.Ed25519Signer
import org.hyperledger.iroha.sdk.crypto.KeyGenerationOutcome
import org.hyperledger.iroha.sdk.crypto.KeyManagementException
import org.hyperledger.iroha.sdk.crypto.KeyProvider
import org.hyperledger.iroha.sdk.crypto.KeyProviderMetadata
import org.hyperledger.iroha.sdk.crypto.SigningException
import org.hyperledger.iroha.sdk.crypto.Signer
import org.hyperledger.iroha.sdk.crypto.SoftwareKeyProvider
import org.hyperledger.iroha.sdk.crypto.export.KeyExportBundle
import org.hyperledger.iroha.sdk.crypto.export.KeyExportException
import org.hyperledger.iroha.sdk.crypto.export.KeyExportStore
import org.hyperledger.iroha.sdk.crypto.export.KeyPassphraseProvider
import org.hyperledger.iroha.sdk.crypto.keystore.KeyAttestation
import org.hyperledger.iroha.sdk.crypto.keystore.KeyGenParameters
import org.hyperledger.iroha.sdk.crypto.keystore.KeySecurityPreference
import org.hyperledger.iroha.sdk.crypto.keystore.KeystoreKeyProvider
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AttestationResult
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AttestationVerificationException
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AttestationVerifier
import org.hyperledger.iroha.sdk.telemetry.KeystoreTelemetryEmitter

private val ED25519_SPKI_PREFIX = byteArrayOf(
    0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00
)
private const val ED25519_SPKI_SIZE = 44

/**
 * Coordinates key generation and lookup for Iroha Android clients.
 *
 * The manager accepts one or more `KeyProvider` implementations and routes requests according
 * to the supplied `KeySecurityPreference`. When a hardware-backed provider is unavailable, the
 * manager falls back to software providers so developers can continue testing on emulators and
 * desktop JVMs.
 */
class IrohaKeyManager private constructor(
    providers: List<KeyProvider>,
    private val keystoreTelemetry: KeystoreTelemetryEmitter,
) {
    private val providers: List<KeyProvider> = providers.toList()

    init {
        require(this.providers.isNotEmpty()) { "At least one KeyProvider is required" }
    }

    /**
     * Generates or loads a key associated with `alias` honouring the requested security preference.
     *
     * @throws KeyManagementException when no provider can satisfy the request
     */
    @Throws(KeyManagementException::class)
    fun generateOrLoad(alias: String, preference: KeySecurityPreference): KeyPair {
        require(alias.isNotBlank()) { "alias must not be blank" }

        val ordered = orderedProviders(preference)
        var lastError: KeyManagementException? = null
        for (provider in ordered) {
            try {
                val existing = provider.load(alias)
                if (existing != null) {
                    ensureEd25519KeyPair(alias, preference, existing, provider.metadata(), "load")
                    return existing
                }
            } catch (e: KeyManagementException) {
                lastError = e
            }
        }

        for (provider in ordered) {
            try {
                val keyPair = provider.generate(alias)
                val route = routeFromMetadata(provider.metadata())
                val outcome = KeyGenerationOutcome(keyPair, route)
                ensureEd25519KeyPair(alias, preference, outcome.keyPair, provider.metadata(), "generate")
                enforcePreference(preference, provider.metadata(), outcome)
                recordKeyGenerationTelemetry(alias, preference, provider.metadata(), outcome)
                return outcome.keyPair
            } catch (e: KeyManagementException) {
                lastError = e
            }
        }
        if (lastError != null) throw lastError
        throw KeyManagementException("No key providers available for alias=$alias")
    }

    private fun enforcePreference(
        preference: KeySecurityPreference,
        metadata: KeyProviderMetadata,
        outcome: KeyGenerationOutcome,
    ) {
        if (preference == KeySecurityPreference.STRONGBOX_REQUIRED
            && outcome.route != KeyGenerationOutcome.Route.STRONGBOX) {
            throw KeyManagementException(
                "StrongBox required but provider ${metadata.name} generated key using ${outcome.route}")
        }
        if (preference == KeySecurityPreference.HARDWARE_REQUIRED
            && outcome.route == KeyGenerationOutcome.Route.SOFTWARE) {
            throw KeyManagementException(
                "Hardware-backed key required but provider ${metadata.name} generated software key")
        }
    }

    private fun recordKeyGenerationTelemetry(
        alias: String,
        preference: KeySecurityPreference,
        metadata: KeyProviderMetadata,
        outcome: KeyGenerationOutcome,
    ) {
        val fallback = (preference == KeySecurityPreference.STRONGBOX_REQUIRED
            || preference == KeySecurityPreference.STRONGBOX_PREFERRED)
            && outcome.route != KeyGenerationOutcome.Route.STRONGBOX
        keystoreTelemetry.recordKeyGeneration(alias, preference?.name, metadata, outcome.route, fallback)
    }

    private fun ensureEd25519KeyPair(
        alias: String?,
        preference: KeySecurityPreference?,
        keyPair: KeyPair?,
        metadata: KeyProviderMetadata?,
        phase: String,
    ) {
        val validation = validateEd25519KeyPair(keyPair)
        if (!validation.valid) {
            recordKeyValidationFailure(alias, preference, metadata, phase, validation)
            val provider = metadata?.name ?: "unknown"
            throw KeyManagementException(
                "Provider $provider returned non-Ed25519 key material (${validation.detail()})")
        }
    }

    private fun recordKeyValidationFailure(
        alias: String?,
        preference: KeySecurityPreference?,
        metadata: KeyProviderMetadata?,
        phase: String,
        validation: Ed25519SpkiValidation,
    ) {
        keystoreTelemetry.recordKeyValidationFailure(
            alias, preference?.name, metadata, phase, validation.reason,
            validation.length, ED25519_SPKI_SIZE, validation.prefixHex,
        )
    }

    /**
     * Generates an ephemeral key pair suitable for offline transaction signing.
     *
     * Ephemeral keys are never persisted; providers may favour software-backed generation even
     * when hardware-backed providers are present to avoid exhausting secure hardware key slots.
     */
    @Throws(KeyManagementException::class)
    fun generateEphemeral(): KeyPair {
        var lastError: KeyManagementException? = null
        for (provider in providers) {
            try {
                val keyPair = provider.generateEphemeral()
                ensureEd25519KeyPair(null, null, keyPair, provider.metadata(), "ephemeral")
                return keyPair
            } catch (e: KeyManagementException) {
                lastError = e
            }
        }
        if (lastError != null) throw lastError
        throw KeyManagementException("No key providers available for ephemeral keys")
    }

    /**
     * Produces a signer bound to the key referenced by `alias`. The key is lazily created if it
     * does not exist yet, matching `generateOrLoad` semantics.
     */
    @Throws(KeyManagementException::class, SigningException::class)
    fun signerForAlias(alias: String, preference: KeySecurityPreference): Signer {
        val keyPair = generateOrLoad(alias, preference)
        return Ed25519Signer(keyPair.private, keyPair.public)
    }

    /** Returns metadata for each configured key provider in priority order. */
    fun providerMetadata(): List<KeyProviderMetadata> =
        providers.map { it.metadata() }

    /** Returns `true` if any provider advertises hardware-backed support. */
    fun hasHardwareBackedProvider(): Boolean =
        providers.any { it.metadata().hardwareBacked }

    /** Returns `true` when a StrongBox-backed provider is registered. */
    fun hasStrongBoxProvider(): Boolean =
        providers.any { it.metadata().strongBoxBacked }

    /**
     * Exports the software-backed key referenced by `alias` using deterministic HKDF + AES-GCM
     * derivation. The passphrase is consumed as UTF-8 and cleared after derivation.
     *
     * @throws KeyExportException when the software provider cannot export the key material
     * @throws KeyManagementException when the alias is unknown or no software provider is available
     */
    @Throws(KeyManagementException::class, KeyExportException::class)
    fun exportDeterministicKey(alias: String, passphrase: CharArray): KeyExportBundle =
        softwareProvider().exportDeterministic(alias, passphrase)

    /**
     * Imports the provided deterministic export into the software provider, replacing any existing key
     * registered under `bundle.alias()`.
     *
     * @throws KeyExportException when the bundle cannot be decoded
     * @throws KeyManagementException when no software provider is available
     */
    @Throws(KeyExportException::class, KeyManagementException::class)
    fun importDeterministicKey(bundle: KeyExportBundle, passphrase: CharArray): KeyPair =
        softwareProvider().importDeterministic(bundle, passphrase)

    /**
     * Verifies attestation material produced by hardware-backed providers for `alias`.
     *
     * The first provider that returns a non-empty attestation result is treated as authoritative.
     * Providers that do not expose attestation simply return null.
     *
     * @param alias alias whose attestation should be verified
     * @param verifier verifier configured with trusted roots and policy expectations
     * @param expectedChallenge optional challenge value that must match the attested payload when provided
     * @return the attestation verification result when available, or null when no attestation is recorded for `alias`
     * @throws AttestationVerificationException when attestation verification fails for the alias
     */
    @Throws(AttestationVerificationException::class)
    fun verifyAttestation(
        alias: String,
        verifier: AttestationVerifier,
        expectedChallenge: ByteArray? = null,
    ): AttestationResult? {
        for (provider in providers) {
            try {
                val result = provider.verifyAttestation(alias, verifier, expectedChallenge)
                if (result != null) {
                    keystoreTelemetry.recordResult(alias, provider.metadata(), result.attestationSecurityLevel.name, result.leafCertificate)
                    return result
                }
            } catch (ex: AttestationVerificationException) {
                keystoreTelemetry.recordFailure(alias, provider.metadata(), ex.message)
                throw ex
            }
        }
        return null
    }

    /**
     * Requests fresh attestation material for `alias`. Providers that do not support
     * attestation generation return null.
     *
     * @param alias alias to attest
     * @param challenge attestation challenge (may be null if provider does not require it)
     * @return attestation bundle when generated, otherwise null
     * @throws KeyManagementException when provider-specific errors occur during attestation
     */
    @Throws(KeyManagementException::class)
    fun generateAttestation(alias: String, challenge: ByteArray?): KeyAttestation? {
        require(alias.isNotBlank()) { "alias must not be blank" }

        val ordered = providers.sortedByDescending { it.metadata().supportsAttestationCertificates }

        var lastError: KeyManagementException? = null
        for (provider in ordered) {
            try {
                val clonedChallenge = challenge?.clone()
                val attestation = provider.generateAttestation(alias, clonedChallenge)
                if (attestation != null) return attestation
            } catch (e: KeyManagementException) {
                keystoreTelemetry.recordFailure(alias, provider.metadata(), e.message)
                lastError = e
            }
        }
        if (lastError != null) throw lastError
        return null
    }

    /** Returns a copy of this manager that emits keystore telemetry through `telemetry`. */
    fun withTelemetry(telemetry: KeystoreTelemetryEmitter): IrohaKeyManager =
        IrohaKeyManager(providers, telemetry)

    private fun orderedProviders(preference: KeySecurityPreference): List<KeyProvider> {
        val ordered = providers.toMutableList()
        when (preference) {
            KeySecurityPreference.STRONGBOX_REQUIRED ->
                ordered.removeAll { !it.metadata().strongBoxBacked }
            KeySecurityPreference.STRONGBOX_PREFERRED ->
                ordered.sortWith(
                    compareByDescending<KeyProvider> { it.metadata().strongBoxBacked }
                        .thenByDescending { it.metadata().hardwareBacked }
                )
            KeySecurityPreference.HARDWARE_REQUIRED ->
                ordered.removeAll { !it.metadata().hardwareBacked }
            KeySecurityPreference.HARDWARE_PREFERRED ->
                ordered.sortByDescending { it.metadata().hardwareBacked }
            KeySecurityPreference.SOFTWARE_ONLY ->
                ordered.removeAll { it.metadata().hardwareBacked }
        }
        return ordered
    }

    private fun softwareProvider(): SoftwareKeyProvider {
        for (provider in providers) {
            if (provider is SoftwareKeyProvider) return provider
        }
        throw KeyManagementException("No software key provider available for deterministic export")
    }

    companion object {
        /** Creates a manager that uses the provided providers in priority order. */
        @JvmStatic
        fun fromProviders(providers: List<KeyProvider>): IrohaKeyManager =
            IrohaKeyManager(providers, KeystoreTelemetryEmitter.noop())

        /** Creates a manager with explicit keystore telemetry configuration. */
        @JvmStatic
        fun fromProviders(providers: List<KeyProvider>, telemetry: KeystoreTelemetryEmitter): IrohaKeyManager =
            IrohaKeyManager(providers, telemetry)

        /** Creates a manager with a software fallback provider only (desktop/emulator friendly). */
        @JvmStatic
        fun withSoftwareFallback(): IrohaKeyManager =
            IrohaKeyManager(listOf(SoftwareKeyProvider()), KeystoreTelemetryEmitter.noop())

        /**
         * Creates a manager backed by an exportable software provider that persists deterministic key
         * exports using `exportStore`.
         */
        @JvmStatic
        fun withExportableSoftwareKeys(
            exportStore: KeyExportStore,
            passphraseProvider: KeyPassphraseProvider,
        ): IrohaKeyManager = IrohaKeyManager(
            listOf(
                SoftwareKeyProvider(
                    SoftwareKeyProvider.ProviderPolicy.BOUNCY_CASTLE_REQUIRED,
                    exportStore,
                    passphraseProvider,
                ),
            ),
            KeystoreTelemetryEmitter.noop(),
        )

        /**
         * Creates a manager that attempts to use hardware-backed keystore providers (when available) and
         * falls back to the software provider for emulators/desktop JVMs.
         */
        @JvmStatic
        fun withDefaultProviders(): IrohaKeyManager =
            withDefaultProviders(KeyGenParameters.builder().build())

        /**
         * Creates a manager that attempts to use hardware-backed keystore providers with the supplied
         * generation parameters and falls back to a software provider.
         */
        @JvmStatic
        fun withDefaultProviders(keyGenParameters: KeyGenParameters): IrohaKeyManager {
            val providers = mutableListOf<KeyProvider>()
            KeystoreKeyProvider.maybeCreate(keyGenParameters)?.let { providers.add(it) }
            providers.add(SoftwareKeyProvider())
            return IrohaKeyManager(providers, KeystoreTelemetryEmitter.noop())
        }

        /**
         * Creates a manager that attempts to use hardware-backed keystore providers with telemetry and
         * falls back to a software provider.
         */
        @JvmStatic
        fun withDefaultProviders(
            keyGenParameters: KeyGenParameters,
            telemetry: KeystoreTelemetryEmitter,
        ): IrohaKeyManager {
            val providers = mutableListOf<KeyProvider>()
            KeystoreKeyProvider.maybeCreate(keyGenParameters)?.let { providers.add(it) }
            providers.add(SoftwareKeyProvider())
            return IrohaKeyManager(providers, telemetry)
        }

        private fun routeFromMetadata(metadata: KeyProviderMetadata?): KeyGenerationOutcome.Route {
            if (metadata == null) return KeyGenerationOutcome.Route.SOFTWARE
            if (metadata.strongBoxBacked) return KeyGenerationOutcome.Route.STRONGBOX
            if (metadata.hardwareBacked) return KeyGenerationOutcome.Route.HARDWARE
            return KeyGenerationOutcome.Route.SOFTWARE
        }

        private fun validateEd25519KeyPair(keyPair: KeyPair?): Ed25519SpkiValidation {
            if (keyPair?.public == null) {
                return Ed25519SpkiValidation.invalid(0, "", "public_key_missing")
            }
            return validateEd25519Spki(keyPair.public.encoded)
        }

        private fun validateEd25519Spki(encoded: ByteArray?): Ed25519SpkiValidation {
            if (encoded == null || encoded.isEmpty()) {
                return Ed25519SpkiValidation.invalid(0, "", "spki_missing")
            }
            val bytes = encoded!!
            val length = bytes.size
            val prefixLen = minOf(ED25519_SPKI_PREFIX.size, length)
            val prefixHex = toHex(bytes, prefixLen)
            if (length != ED25519_SPKI_SIZE) {
                return Ed25519SpkiValidation.invalid(length, prefixHex, "length_mismatch")
            }
            for (i in ED25519_SPKI_PREFIX.indices) {
                if (bytes[i] != ED25519_SPKI_PREFIX[i]) {
                    return Ed25519SpkiValidation.invalid(length, prefixHex, "prefix_mismatch")
                }
            }
            return Ed25519SpkiValidation.valid(length, prefixHex)
        }

        private fun toHex(bytes: ByteArray?, length: Int): String {
            if (bytes == null || length <= 0) return ""
            val limit = minOf(bytes.size, length)
            return (0 until limit).joinToString("") { "%02x".format(bytes[it]) }
        }
    }

    private class Ed25519SpkiValidation private constructor(
        val valid: Boolean,
        val length: Int,
        val prefixHex: String,
        val reason: String,
    ) {
        fun detail(): String =
            "reason=$reason, spki_len=$length, expected_len=$ED25519_SPKI_SIZE, prefix=${prefixHex.ifEmpty { "unknown" }}"

        companion object {
            fun valid(length: Int, prefixHex: String): Ed25519SpkiValidation =
                Ed25519SpkiValidation(true, length, prefixHex, "ok")

            fun invalid(length: Int, prefixHex: String, reason: String): Ed25519SpkiValidation =
                Ed25519SpkiValidation(false, length, prefixHex, reason)
        }
    }
}

/**
 * Extension methods on `KeyProvider` that adapt the simpler interface to the richer
 * operations required by `IrohaKeyManager`. These default implementations delegate to
 * the existing `KeyProvider` contract.
 */
private fun KeyProvider.verifyAttestation(
    alias: String,
    verifier: AttestationVerifier,
    expectedChallenge: ByteArray?,
): AttestationResult? = null

private fun KeyProvider.generateAttestation(alias: String, challenge: ByteArray?): KeyAttestation? = null
