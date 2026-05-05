package org.hyperledger.iroha.sdk.crypto.keystore

import java.security.KeyPair
import org.hyperledger.iroha.sdk.crypto.KeyManagementException
import org.hyperledger.iroha.sdk.crypto.KeyProviderMetadata

/**
 * Facade over the Android Keystore (and StrongBox) primitives.
 *
 * This abstraction allows the desktop JVM build to compile without depending on the Android SDK
 * while enabling platform-specific backends in instrumentation builds. The default implementation
 * will land alongside the Android integration and will delegate to `android.security.keystore`
 * APIs.
 */
interface KeystoreBackend {

    /** Load an existing key identified by `alias`. */
    @Throws(KeyManagementException::class)
    fun load(alias: String): KeyPair?

    /** Generate and persist a key for `alias`. */
    @Throws(KeyManagementException::class)
    fun generate(alias: String, parameters: KeyGenParameters): KeyGenerationResult

    /** Generate a transient key that must not be persisted. */
    @Throws(KeyManagementException::class)
    fun generateEphemeral(parameters: KeyGenParameters): KeyPair

    /** Metadata describing the backing store/hardware. */
    fun metadata(): KeyProviderMetadata

    /** Human readable backend name (e.g., `android-keystore`). */
    fun name(): String

    /**
     * Returns the attestation material for `alias`, if available.
     *
     * Backends should return null when attestation is unsupported or the alias has no
     * recorded certificates. The Android implementation will populate this with the StrongBox/TEE
     * attestation chain.
     */
    fun attestation(alias: String): KeyAttestation? = null

    /**
     * Generates fresh attestation material for `alias` using the provided challenge. Providers
     * that do not support attestation should return null.
     */
    @Throws(KeyManagementException::class)
    fun generateAttestation(alias: String, challenge: ByteArray): KeyAttestation? = null
}
