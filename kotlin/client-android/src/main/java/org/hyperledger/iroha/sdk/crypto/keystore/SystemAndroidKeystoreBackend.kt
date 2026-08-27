package org.hyperledger.iroha.sdk.crypto.keystore

import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyInfo
import android.security.keystore.KeyProperties
import java.io.IOException
import java.security.GeneralSecurityException
import java.security.KeyFactory
import java.security.KeyPair
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.ProviderException
import java.security.cert.Certificate
import java.security.cert.X509Certificate
import java.security.spec.NamedParameterSpec
import org.hyperledger.iroha.sdk.crypto.KeyManagementException
import org.hyperledger.iroha.sdk.crypto.KeyProviderMetadata

private const val ANDROID_KEYSTORE = "AndroidKeyStore"

/**
 * `AndroidKeystoreBackend` implementation that bridges to the platform Android Keystore.
 */
internal class SystemAndroidKeystoreBackend private constructor(
    private val _metadata: KeyProviderMetadata,
) : AndroidKeystoreBackend {

    @Throws(KeyManagementException::class)
    override fun load(alias: String): KeyPair? {
        require(alias.isNotBlank()) { "alias must not be blank" }
        try {
            val keyStore = loadKeyStore()
            if (!keyStore.containsAlias(alias)) return null
            val entry = keyStore.getEntry(alias, null)
            if (entry !is KeyStore.PrivateKeyEntry) return null
            return KeyPair(entry.certificate.publicKey, entry.privateKey)
        } catch (ex: GeneralSecurityException) {
            throw KeyManagementException("Failed to load key from Android Keystore", ex)
        } catch (ex: IOException) {
            throw KeyManagementException("Failed to load key from Android Keystore", ex)
        }
    }

    @Throws(KeyManagementException::class)
    override fun generate(alias: String, parameters: KeyGenParameters): KeyGenerationResult {
        require(alias.isNotBlank()) { "alias must not be blank" }
        val generator = createKeyPairGenerator(parameters.algorithm)
        val strongBoxRequested = parameters.requireStrongBox || parameters.preferStrongBox
        return generateInternal(generator, alias, parameters, strongBoxRequested)
    }

    private fun generateInternal(
        generator: KeyPairGenerator,
        alias: String,
        parameters: KeyGenParameters,
        strongBoxRequested: Boolean,
    ): KeyGenerationResult {
        val spec: KeyGenParameterSpec
        try {
            spec = buildKeyGenParameterSpec(alias, parameters, strongBoxRequested)
        } catch (ex: GeneralSecurityException) {
            throw KeyManagementException("Failed to prepare Android Keystore parameters", ex)
        }

        try {
            generator.initialize(spec)
            val pair = generator.generateKeyPair()
            return KeyGenerationResult(pair, keyMetadata(alias, pair).strongBoxBacked)
        } catch (ex: ProviderException) {
            throw KeyManagementException("Android Keystore generation failed", ex)
        } catch (ex: GeneralSecurityException) {
            if (parameters.algorithm.equals("Ed25519", ignoreCase = true)) {
                throw KeyManagementException(
                    "Android Keystore does not support hardware Ed25519 key generation on this device",
                    ex
                )
            }
            throw KeyManagementException("Android Keystore generation failed", ex)
        }
    }

    @Throws(KeyManagementException::class)
    override fun generateEphemeral(parameters: KeyGenParameters): KeyPair =
        throw KeyManagementException("Android Keystore does not support unmanaged ephemeral keys")

    override fun metadata(): KeyProviderMetadata = _metadata

    @Suppress("DEPRECATION")
    override fun keyMetadata(alias: String, keyPair: KeyPair): KeyProviderMetadata {
        require(alias.isNotBlank()) { "alias must not be blank" }
        val level = try {
            val factory = KeyFactory.getInstance(keyPair.private.algorithm, ANDROID_KEYSTORE)
            val keyInfo = factory.getKeySpec(keyPair.private, KeyInfo::class.java)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                when (keyInfo.securityLevel) {
                    KeyProperties.SECURITY_LEVEL_STRONGBOX ->
                        KeyProviderMetadata.HardwareSecurityLevel.STRONGBOX
                    KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT,
                    KeyProperties.SECURITY_LEVEL_UNKNOWN_SECURE ->
                        KeyProviderMetadata.HardwareSecurityLevel.TRUSTED_ENVIRONMENT
                    else -> KeyProviderMetadata.HardwareSecurityLevel.NONE
                }
            } else if (keyInfo.isInsideSecureHardware) {
                KeyProviderMetadata.HardwareSecurityLevel.TRUSTED_ENVIRONMENT
            } else {
                KeyProviderMetadata.HardwareSecurityLevel.NONE
            }
        } catch (_: GeneralSecurityException) {
            KeyProviderMetadata.HardwareSecurityLevel.NONE
        } catch (_: RuntimeException) {
            KeyProviderMetadata.HardwareSecurityLevel.NONE
        }
        return when (level) {
            KeyProviderMetadata.HardwareSecurityLevel.STRONGBOX ->
                KeyProviderMetadata.strongBox(
                    _metadata.name,
                    _metadata.supportsAttestationCertificates,
                )
            KeyProviderMetadata.HardwareSecurityLevel.TRUSTED_ENVIRONMENT ->
                KeyProviderMetadata(
                    name = _metadata.name,
                    hardwareBacked = true,
                    supportsAttestationCertificates = _metadata.supportsAttestationCertificates,
                    securityLevel = level,
                )
            else -> KeyProviderMetadata(
                name = _metadata.name,
                supportsAttestationCertificates = _metadata.supportsAttestationCertificates,
            )
        }
    }

    override fun name(): String = _metadata.name

    @Throws(KeyManagementException::class)
    override fun attestation(alias: String): KeyAttestation? {
        return try {
            loadAttestationBundle(alias)
        } catch (ex: GeneralSecurityException) {
            throw KeyManagementException("Failed to read Android Keystore attestation", ex)
        } catch (ex: IOException) {
            throw KeyManagementException("Failed to read Android Keystore attestation", ex)
        }
    }

    @Throws(KeyManagementException::class)
    override fun generateAttestation(alias: String, challenge: ByteArray): KeyAttestation? {
        require(alias.isNotBlank()) { "alias must not be blank" }
        val challengeCopy = challenge.copyOf()
        try {
            val keyStore = loadKeyStore()
            if (!keyStore.containsAlias(alias)) return null
            if (challengeCopy.isNotEmpty()) {
                throw KeyManagementException(
                    "Android Keystore cannot re-attest an existing alias; provision a new alias " +
                        "with KeyGenParameters.setAttestationChallenge"
                )
            }
            return loadAttestationBundle(alias, keyStore)
        } catch (ex: GeneralSecurityException) {
            throw KeyManagementException("Failed to read Android Keystore attestation", ex)
        } catch (ex: IOException) {
            throw KeyManagementException("Failed to read Android Keystore attestation", ex)
        }
    }

    private fun loadAttestationBundle(alias: String): KeyAttestation? {
        val keyStore = loadKeyStore()
        return loadAttestationBundle(alias, keyStore)
    }

    private fun loadAttestationBundle(alias: String, keyStore: KeyStore): KeyAttestation? {
        val chain: Array<Certificate>? = keyStore.getCertificateChain(alias)
        return buildAttestation(alias, chain)
    }

    private fun buildAttestation(alias: String, chain: Array<Certificate>?): KeyAttestation? {
        if (chain.isNullOrEmpty()) return null
        val builder = KeyAttestation.builder().setAlias(alias)
        for (certificate in chain) {
            if (certificate is X509Certificate) {
                builder.addCertificate(certificate)
            }
        }
        return builder.build()
    }

    companion object {
        fun create(): AndroidKeystoreBackend? {
            if (!isAndroidRuntime()) return null
            val keyStore: KeyStore
            try {
                keyStore = KeyStore.getInstance(ANDROID_KEYSTORE)
                keyStore.load(null)
            } catch (_: GeneralSecurityException) {
                return null
            } catch (_: IOException) {
                return null
            }

            val supportsStrongBox = detectStrongBoxSupport(keyStore)

            val metadata = if (supportsStrongBox) {
                KeyProviderMetadata(
                    name = "android-keystore",
                    hardwareBacked = true,
                    strongBoxBacked = true,
                    supportsAttestationCertificates = true,
                    securityLevel = KeyProviderMetadata.HardwareSecurityLevel.STRONGBOX,
                )
            } else {
                KeyProviderMetadata(
                    name = "android-keystore",
                    hardwareBacked = true,
                    supportsAttestationCertificates = true,
                    securityLevel = KeyProviderMetadata.HardwareSecurityLevel.TRUSTED_ENVIRONMENT,
                )
            }

            return SystemAndroidKeystoreBackend(metadata)
        }

        private fun isAndroidRuntime(): Boolean =
            try {
                Build.VERSION.SDK_INT
                true
            } catch (_: Throwable) {
                false
            }

        private fun loadKeyStore(): KeyStore {
            val keyStore = KeyStore.getInstance(ANDROID_KEYSTORE)
            keyStore.load(null)
            return keyStore
        }

        private fun createKeyPairGenerator(algorithm: String?): KeyPairGenerator {
            val resolvedAlgorithm = algorithm ?: "Ed25519"
            try {
                return KeyPairGenerator.getInstance(resolvedAlgorithm, ANDROID_KEYSTORE)
            } catch (ex: GeneralSecurityException) {
                throw KeyManagementException(
                    "Android Keystore does not support algorithm $resolvedAlgorithm", ex
                )
            }
        }

        private fun buildKeyGenParameterSpec(
            alias: String,
            parameters: KeyGenParameters,
            strongBox: Boolean,
        ): KeyGenParameterSpec {
            val purposes = KeyProperties.PURPOSE_SIGN or KeyProperties.PURPOSE_VERIFY
            val builder = KeyGenParameterSpec.Builder(alias, purposes)
                .setDigests(KeyProperties.DIGEST_NONE)
            setAlgorithmParameterSpecIfNeeded(builder, parameters.algorithm)

            if (parameters.userAuthenticationRequired) {
                builder.setUserAuthenticationRequired(true)
                val timeout = parameters.userAuthenticationTimeout
                var seconds = timeout?.seconds ?: 0L
                if (seconds < 0L) seconds = 0L
                val safeSeconds = seconds.coerceIn(0L, Int.MAX_VALUE.toLong()).toInt()
                if (Build.VERSION.SDK_INT >= 30) {
                    builder.setUserAuthenticationParameters(safeSeconds, KeyProperties.AUTH_BIOMETRIC_STRONG or KeyProperties.AUTH_DEVICE_CREDENTIAL)
                }
            }

            if (Build.VERSION.SDK_INT >= 28) {
                builder.setIsStrongBoxBacked(strongBox)
            }

            val challenge = parameters.attestationChallenge()
            if (challenge != null && challenge.isNotEmpty()) {
                builder.setAttestationChallenge(challenge.copyOf())
            }

            val usageCountLimit = parameters.usageCountLimit
            if (usageCountLimit != null) {
                if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S) {
                    throw GeneralSecurityException("Usage count limits are not supported on this Android API level")
                }
                builder.setMaxUsageCount(usageCountLimit)
            }

            return builder.build()
        }

        private fun setAlgorithmParameterSpecIfNeeded(
            builder: KeyGenParameterSpec.Builder,
            algorithm: String?,
        ) {
            if (!algorithm.equals("Ed25519", ignoreCase = true)) return
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) {
                throw GeneralSecurityException(
                    "Android Keystore Ed25519 parameters require API 33 or newer"
                )
            }
            builder.setAlgorithmParameterSpec(NamedParameterSpec("Ed25519"))
        }

        private fun detectStrongBoxSupport(keyStore: KeyStore): Boolean {
            if (Build.VERSION.SDK_INT < 28) return false
            return try {
                val generator = KeyPairGenerator.getInstance("Ed25519", ANDROID_KEYSTORE)
                val parameters = KeyGenParameters.builder().setRequireStrongBox(true).build()
                val spec = buildKeyGenParameterSpec("__iroha_strongbox_probe__", parameters, true)
                generator.initialize(spec)
                cleanupProbeAlias(keyStore)
                true
            } catch (_: ProviderException) {
                false
            } catch (_: GeneralSecurityException) {
                false
            }
        }

        private fun cleanupProbeAlias(keyStore: KeyStore) {
            try {
                if (keyStore.containsAlias("__iroha_strongbox_probe__")) {
                    keyStore.deleteEntry("__iroha_strongbox_probe__")
                }
            } catch (_: GeneralSecurityException) {
                // Best-effort cleanup.
            }
        }
    }
}
