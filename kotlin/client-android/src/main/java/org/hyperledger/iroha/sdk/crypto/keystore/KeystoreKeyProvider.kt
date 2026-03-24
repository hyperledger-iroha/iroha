package org.hyperledger.iroha.sdk.crypto.keystore

import java.security.KeyPair
import java.util.concurrent.ConcurrentHashMap
import org.hyperledger.iroha.sdk.crypto.KeyGenerationOutcome
import org.hyperledger.iroha.sdk.crypto.KeyManagementException
import org.hyperledger.iroha.sdk.crypto.KeyProvider
import org.hyperledger.iroha.sdk.crypto.KeyProviderMetadata
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AttestationResult
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AttestationVerificationException
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AttestationVerifier

/**
 * `KeyProvider` backed by an Android Keystore-compatible backend.
 *
 * The backend is supplied through `KeystoreBackend`; desktop JVM builds can rely on fake
 * implementations for tests while Android builds will provide a real backend that bridges to
 * `android.security.keystore.KeyStore` / `KeyGenParameterSpec`. Support for StrongBox
 * and discrete secure elements will land by swapping the backend at runtime depending on device
 * capabilities.
 */
class KeystoreKeyProvider(
    private val backend: KeystoreBackend,
    private val parameters: KeyGenParameters,
) : KeyProvider {

    private val attestationCache = ConcurrentHashMap<CacheKey, KeyAttestation>()

    @Throws(KeyManagementException::class)
    override fun load(alias: String): KeyPair? = backend.load(alias)

    @Throws(KeyManagementException::class)
    override fun generate(alias: String): KeyPair {
        evictAttestations(alias)
        val result = backend.generate(alias, parameters)
        if (parameters.requireStrongBox
            && !result.strongBoxBacked
            && !backend.metadata().strongBoxBacked
        ) {
            throw KeyManagementException("StrongBox required but backend is not StrongBox-capable")
        }
        return result.keyPair
    }

    @Throws(KeyManagementException::class)
    fun generate(alias: String, preference: KeySecurityPreference): KeyPair {
        evictAttestations(alias)
        val effective = parametersFor(preference)
        if (effective.requireStrongBox && !backend.metadata().strongBoxBacked) {
            throw KeyManagementException("StrongBox required but backend is not StrongBox-capable")
        }
        val result = backend.generate(alias, effective)
        if (preference == KeySecurityPreference.STRONGBOX_REQUIRED && !result.strongBoxBacked) {
            throw KeyManagementException(
                "StrongBox required but backend fell back to a weaker security level"
            )
        }
        return result.keyPair
    }

    @Throws(KeyManagementException::class)
    fun generateWithOutcome(
        alias: String,
        preference: KeySecurityPreference,
    ): KeyGenerationOutcome {
        evictAttestations(alias)
        val effective = parametersFor(preference)
        val result = backend.generate(alias, effective)
        val route = when {
            result.strongBoxBacked -> KeyGenerationOutcome.Route.STRONGBOX
            backend.metadata().hardwareBacked -> KeyGenerationOutcome.Route.HARDWARE
            else -> KeyGenerationOutcome.Route.SOFTWARE
        }
        if (preference == KeySecurityPreference.STRONGBOX_REQUIRED
            && route != KeyGenerationOutcome.Route.STRONGBOX
        ) {
            throw KeyManagementException(
                "StrongBox required but backend fell back to a weaker security level"
            )
        }
        return KeyGenerationOutcome(result.keyPair, route)
    }

    @Throws(KeyManagementException::class)
    override fun generateEphemeral(): KeyPair = backend.generateEphemeral(parameters)

    override fun isHardwareBacked(): Boolean = backend.metadata().hardwareBacked

    override fun metadata(): KeyProviderMetadata = backend.metadata()

    override fun name(): String = backend.name()

    /** Returns attestation material recorded for `alias`, when available. */
    fun attestation(alias: String): KeyAttestation? =
        try {
            fetchAttestation(alias, NO_CHALLENGE)
        } catch (_: KeyManagementException) {
            null
        }

    /**
     * Requests fresh attestation for `alias`. Providers that do not support attestation return
     * null.
     */
    @Throws(KeyManagementException::class)
    fun generateAttestation(alias: String, challenge: ByteArray?): KeyAttestation? {
        val normalizedChallenge = challenge?.copyOf() ?: NO_CHALLENGE
        val fingerprint = fingerprintChallenge(normalizedChallenge)
        if (normalizedChallenge.isEmpty()) {
            val cached = lookupCachedAttestation(alias, normalizedChallenge)
            if (cached != null) return cached
            val existing = backend.attestation(alias)
            if (existing != null) return cacheAttestation(alias, normalizedChallenge, existing)
            return backend.generateAttestation(alias, normalizedChallenge)
                ?.let { cacheAttestation(alias, normalizedChallenge, it) }
        }
        evictAttestationEntry(alias, fingerprint)
        val attestation = backend.generateAttestation(alias, normalizedChallenge)
            ?: throw KeyManagementException(
                "Attestation challenge is not supported by backend ${backend.name()}"
            )
        return cacheAttestation(alias, normalizedChallenge, attestation)
    }

    /**
     * Verifies attestation material (when present) using the supplied verifier.
     *
     * Returns null when the backend has no attestation for `alias`.
     */
    @Throws(AttestationVerificationException::class)
    fun verifyAttestation(
        alias: String,
        verifier: AttestationVerifier,
        expectedChallenge: ByteArray?,
    ): AttestationResult? {
        val challenge = expectedChallenge?.copyOf() ?: NO_CHALLENGE
        val attestation = try {
            fetchAttestation(alias, challenge)
        } catch (_: KeyManagementException) {
            return null
        } ?: return null
        try {
            return if (expectedChallenge == null) {
                verifier.verify(attestation)
            } else {
                verifier.verify(attestation, expectedChallenge.copyOf())
            }
        } catch (ex: AttestationVerificationException) {
            evictAttestations(alias)
            throw ex
        }
    }

    /** Verifies attestation without enforcing a challenge match. */
    @Throws(AttestationVerificationException::class)
    fun verifyAttestation(
        alias: String,
        verifier: AttestationVerifier,
    ): AttestationResult? = verifyAttestation(alias, verifier, null)

    /** Returns a copy of this provider with adjusted generation parameters. */
    fun withParameters(parameters: KeyGenParameters): KeystoreKeyProvider =
        KeystoreKeyProvider(backend, parameters)

    /** Returns a copy of this provider tuned for the requested security preference. */
    fun withPreference(preference: KeySecurityPreference): KeystoreKeyProvider =
        KeystoreKeyProvider(backend, parametersFor(preference))

    private fun parametersFor(preference: KeySecurityPreference?): KeyGenParameters {
        if (preference == null) return parameters
        val builder = parameters.toBuilder()
        when (preference) {
            KeySecurityPreference.STRONGBOX_REQUIRED ->
                builder.setRequireStrongBox(true)
                    .setPreferStrongBox(true)
                    .setAllowStrongBoxFallback(false)
            KeySecurityPreference.STRONGBOX_PREFERRED ->
                builder.setRequireStrongBox(false)
                    .setPreferStrongBox(true)
                    .setAllowStrongBoxFallback(true)
            KeySecurityPreference.HARDWARE_REQUIRED ->
                builder.setRequireStrongBox(false)
                    .setPreferStrongBox(false)
                    .setAllowStrongBoxFallback(true)
            KeySecurityPreference.HARDWARE_PREFERRED,
            KeySecurityPreference.SOFTWARE_ONLY -> { /* leave defaults */ }
        }
        return builder.build()
    }

    private fun lookupCachedAttestation(alias: String, challenge: ByteArray): KeyAttestation? {
        val cacheKey = CacheKey(alias, fingerprintChallenge(challenge))
        return attestationCache[cacheKey]
    }

    private fun cacheAttestation(
        alias: String,
        challenge: ByteArray,
        attestation: KeyAttestation,
    ): KeyAttestation {
        val specific = CacheKey(alias, fingerprintChallenge(challenge))
        attestationCache[specific] = attestation
        return attestation
    }

    private fun evictAttestations(alias: String) {
        attestationCache.keys.removeAll { it.alias == alias }
    }

    private fun evictAttestationEntry(alias: String, challengeFingerprint: ByteArray) {
        attestationCache.remove(CacheKey(alias, challengeFingerprint))
    }

    private fun fetchAttestation(alias: String, challenge: ByteArray?): KeyAttestation? {
        val normalized = challenge?.copyOf() ?: NO_CHALLENGE
        val cached = lookupCachedAttestation(alias, normalized)
        if (cached != null) return cached
        if (normalized.isNotEmpty()) {
            val generated = backend.generateAttestation(alias, normalized)
            if (generated != null) return cacheAttestation(alias, normalized, generated)
        }
        return backend.attestation(alias)?.let { cacheAttestation(alias, normalized, it) }
    }

    private class CacheKey(val alias: String, challengeFingerprint: ByteArray) {
        private val _challengeFingerprint: ByteArray

        init {
            require(alias.isNotBlank()) { "alias must not be blank" }
            _challengeFingerprint = challengeFingerprint.copyOf()
        }

        override fun hashCode(): Int {
            var result = alias.hashCode()
            result = 31 * result + _challengeFingerprint.contentHashCode()
            return result
        }

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is CacheKey) return false
            return alias == other.alias
                && _challengeFingerprint.contentEquals(other._challengeFingerprint)
        }
    }

    companion object {
        private val NO_CHALLENGE = ByteArray(0)

        /** Attempts to create a keystore-backed provider when running on Android hardware. */
        @JvmStatic
        fun maybeCreate(parameters: KeyGenParameters): KeystoreKeyProvider? =
            AndroidKeystoreBackend.maybeCreate()?.let { KeystoreKeyProvider(it, parameters) }

        private fun fingerprintChallenge(challenge: ByteArray?): ByteArray {
            if (challenge == null || challenge.isEmpty()) return NO_CHALLENGE
            return challenge.copyOf()
        }
    }
}
