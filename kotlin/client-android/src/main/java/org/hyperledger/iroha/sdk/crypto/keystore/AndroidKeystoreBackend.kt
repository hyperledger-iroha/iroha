package org.hyperledger.iroha.sdk.crypto.keystore

import java.security.KeyPair
import org.hyperledger.iroha.sdk.crypto.KeyManagementException
import org.hyperledger.iroha.sdk.crypto.KeyProviderMetadata

/**
 * Placeholder interface capturing the operations required from the Android Keystore runtime.
 *
 * The actual implementation will live in the Android-specific source set where the Android SDK is
 * available. This marker is provided so the main sources can reference the class without introducing
 * a hard dependency on android.jar in the desktop build.
 */
interface AndroidKeystoreBackend : KeystoreBackend {

    @Throws(KeyManagementException::class)
    override fun load(alias: String): KeyPair?

    @Throws(KeyManagementException::class)
    override fun generate(alias: String, parameters: KeyGenParameters): KeyGenerationResult

    @Throws(KeyManagementException::class)
    override fun generateEphemeral(parameters: KeyGenParameters): KeyPair

    override fun metadata(): KeyProviderMetadata

    override fun name(): String

    override fun attestation(alias: String): KeyAttestation?

    companion object {
        /**
         * Factory that resolves to the platform implementation when running on Android, or returns
         * null otherwise.
         */
        @JvmStatic
        fun maybeCreate(): AndroidKeystoreBackend? = SystemAndroidKeystoreBackend.create()
    }
}
