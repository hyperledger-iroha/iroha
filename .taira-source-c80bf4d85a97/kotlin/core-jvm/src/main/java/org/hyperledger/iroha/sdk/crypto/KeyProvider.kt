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
}
