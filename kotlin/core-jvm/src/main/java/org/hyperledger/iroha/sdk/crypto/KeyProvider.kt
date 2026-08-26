package org.hyperledger.iroha.sdk.crypto

import java.security.KeyPair

/** Common contract for all key provider implementations (software, keystore, etc.). */
interface KeyProvider {

    @Throws(KeyManagementException::class)
    fun load(alias: String): KeyPair?

    @Throws(KeyManagementException::class)
    fun generate(alias: String): KeyPair

    @Throws(KeyManagementException::class)
    fun generateEphemeral(): KeyPair

    fun isHardwareBacked(): Boolean

    fun name(): String

    fun metadata(): KeyProviderMetadata

    /**
     * Reports the measured route for this specific key.
     *
     * Provider metadata describes routing capability and is not proof of a selected key's
     * provenance. Providers that can prove a hardware route must override this method; the
     * conservative default keeps strict hardware policies fail closed.
     */
    @Throws(KeyManagementException::class)
    fun outcomeFor(alias: String, keyPair: KeyPair): KeyGenerationOutcome =
        KeyGenerationOutcome(keyPair, KeyGenerationOutcome.Route.SOFTWARE)
}
