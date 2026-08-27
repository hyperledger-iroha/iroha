package org.hyperledger.iroha.sdk.crypto.keystore

import java.security.KeyPair
import org.hyperledger.iroha.sdk.crypto.KeyManagementException
import org.hyperledger.iroha.sdk.crypto.KeyProviderMetadata

/**
 * Facade over the Android Keystore (and StrongBox) primitives.
 *
 * This abstraction keeps key-provider logic testable while `SystemAndroidKeystoreBackend`
 * delegates to `android.security.keystore` APIs on Android runtimes.
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

    /**
     * Metadata describing the security level of this specific key.
     *
     * Backends whose keys can use different security levels must override this method. The
     * conservative default treats unproven per-key provenance as software-backed so strict
     * hardware policies fail closed.
     */
    @Throws(KeyManagementException::class)
    fun keyMetadata(alias: String, keyPair: KeyPair): KeyProviderMetadata =
        KeyProviderMetadata.software(name())

    /** Human readable backend name (e.g., `android-keystore`). */
    fun name(): String

    /**
     * Returns the attestation material for `alias`, if available.
     *
     * Backends should return null when attestation is unsupported or the alias has no
     * recorded certificates. The Android implementation will populate this with the StrongBox/TEE
     * attestation chain.
     */
    @Throws(KeyManagementException::class)
    fun attestation(alias: String): KeyAttestation? = null

    /**
     * Requests backend-generated attestation material for `alias` using the provided challenge.
     * Providers that do not support challenge-bound generation must fail explicitly for a
     * non-empty challenge. Android Keystore can bind a challenge only while creating a key and
     * cannot re-attest an existing alias.
     */
    @Throws(KeyManagementException::class)
    fun generateAttestation(alias: String, challenge: ByteArray): KeyAttestation? = null
}
