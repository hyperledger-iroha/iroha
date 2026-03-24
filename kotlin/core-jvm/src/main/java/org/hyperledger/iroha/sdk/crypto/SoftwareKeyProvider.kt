package org.hyperledger.iroha.sdk.crypto

import java.security.InvalidParameterException
import java.security.KeyPair
import java.security.KeyPairGenerator
import java.security.NoSuchAlgorithmException
import java.security.NoSuchProviderException
import java.security.Provider
import java.security.SecureRandom
import java.security.Security
import java.util.concurrent.ConcurrentHashMap
import org.hyperledger.iroha.sdk.crypto.export.DeterministicKeyExporter
import org.hyperledger.iroha.sdk.crypto.export.KeyExportBundle
import org.hyperledger.iroha.sdk.crypto.export.KeyExportException
import org.hyperledger.iroha.sdk.crypto.export.KeyExportStore
import org.hyperledger.iroha.sdk.crypto.export.KeyPassphraseProvider

/**
 * JVM friendly key provider that generates Ed25519 key pairs using `KeyPairGenerator`.
 *
 * This provider is intended for desktop tooling, tests, and Android devices without secure
 * elements. It can optionally persist deterministic key exports via a `KeyExportStore` so
 * software-backed accounts can be restored across sessions and devices.
 */
class SoftwareKeyProvider(
    providerPolicy: ProviderPolicy? = ProviderPolicy.DEFAULT,
    private val exportStore: KeyExportStore? = null,
    private val passphraseProvider: KeyPassphraseProvider? = null,
) : KeyProvider {

    /** Controls which JCA provider is used for Ed25519 key generation. */
    enum class ProviderPolicy {
        /** Use the default JCA provider order, falling back to BouncyCastle when needed. */
        DEFAULT,
        /** Prefer BouncyCastle when available to keep keys exportable. */
        BOUNCY_CASTLE_PREFERRED,
        /** Require BouncyCastle; fail if the provider is unavailable. */
        BOUNCY_CASTLE_REQUIRED,
    }

    private val aliasCache = ConcurrentHashMap<String, KeyPair>()
    private val secureRandom = SecureRandom()
    private val providerPolicy: ProviderPolicy = providerPolicy ?: ProviderPolicy.DEFAULT

    init {
        if (exportStore != null && passphraseProvider == null) {
            throw IllegalArgumentException("passphraseProvider is required when exportStore is set")
        }
    }

    constructor() : this(ProviderPolicy.DEFAULT, null, null)

    constructor(providerPolicy: ProviderPolicy) : this(providerPolicy, null, null)

    constructor(
        exportStore: KeyExportStore,
        passphraseProvider: KeyPassphraseProvider,
    ) : this(ProviderPolicy.BOUNCY_CASTLE_PREFERRED, exportStore, passphraseProvider)

    @Throws(KeyManagementException::class)
    override fun load(alias: String): KeyPair? {
        if (alias.isBlank()) return null
        val cached = aliasCache[alias]
        if (cached != null) return cached
        if (exportStore == null) return null
        val restored = loadFromExportStore(alias)
        if (restored != null) aliasCache[alias] = restored
        return restored
    }

    @Throws(KeyManagementException::class)
    override fun generate(alias: String): KeyPair {
        if (alias.isBlank()) {
            throw KeyManagementException("alias must be provided for persistent keys")
        }
        val keyPair = generateKeyPair()
        aliasCache[alias] = keyPair
        persistKey(alias, keyPair)
        return keyPair
    }

    @Throws(KeyManagementException::class)
    override fun generateEphemeral(): KeyPair = generateKeyPair()

    override fun isHardwareBacked(): Boolean = false

    override fun name(): String = "software-key-provider"

    override fun metadata(): KeyProviderMetadata = KeyProviderMetadata.software(name())

    /**
     * Exports the key associated with `alias` deterministically using the supplied
     * `passphrase`.
     */
    @Throws(KeyManagementException::class, KeyExportException::class)
    fun exportDeterministic(alias: String, passphrase: CharArray): KeyExportBundle {
        val keyPair = load(alias)
            ?: throw KeyManagementException("Unknown alias: $alias")
        return DeterministicKeyExporter.exportKeyPair(
            keyPair.private, keyPair.public, alias, passphrase
        )
    }

    /**
     * Imports a deterministic key bundle into the provider, replacing any existing entry for the
     * same alias.
     */
    @Throws(KeyExportException::class)
    fun importDeterministic(bundle: KeyExportBundle, passphrase: CharArray): KeyPair {
        val data = DeterministicKeyExporter.importKeyPair(bundle, passphrase)
        val keyPair = KeyPair(data.publicKey, data.privateKey)
        aliasCache[bundle.alias] = keyPair
        exportStore?.store(bundle.alias, bundle.encodeBase64())
        return keyPair
    }

    private fun generateKeyPair(): KeyPair {
        val generator = newKeyPairGenerator()
        val usedBouncyCastle =
            generator.provider != null && "BC" == generator.provider.name
        val keyPair = generateWithGenerator(generator)
        if (isExportable(keyPair)) return keyPair
        if (!usedBouncyCastle && providerPolicy != ProviderPolicy.BOUNCY_CASTLE_REQUIRED) {
            val fallback = tryBouncyCastleGenerator()
            if (fallback != null) {
                val fallbackPair = generateWithGenerator(fallback)
                if (isExportable(fallbackPair)) return fallbackPair
            }
        }
        throw KeyManagementException(
            "Ed25519 key material is not exportable; use BouncyCastle provider"
        )
    }

    private fun newKeyPairGenerator(): KeyPairGenerator {
        if (providerPolicy == ProviderPolicy.BOUNCY_CASTLE_REQUIRED) {
            return bouncyCastleGeneratorOrThrow()
        }
        if (providerPolicy == ProviderPolicy.BOUNCY_CASTLE_PREFERRED) {
            val preferred = tryBouncyCastleGenerator()
            if (preferred != null) return preferred
        }
        return try {
            KeyPairGenerator.getInstance("Ed25519")
        } catch (_: NoSuchAlgorithmException) {
            val fallback = tryBouncyCastleGenerator()
            if (fallback != null) return fallback
            try {
                KeyPairGenerator.getInstance("EdDSA")
            } catch (ex: NoSuchAlgorithmException) {
                throw KeyManagementException(
                    "Ed25519 key generation is not supported on this JVM", ex
                )
            }
        }
    }

    private fun bouncyCastleGeneratorOrThrow(): KeyPairGenerator =
        tryBouncyCastleGenerator()
            ?: throw KeyManagementException(
                "BouncyCastle provider is required for exportable keys"
            )

    private fun generateWithGenerator(generator: KeyPairGenerator): KeyPair {
        try {
            generator.initialize(255, secureRandom)
        } catch (_: InvalidParameterException) {
            // Providers that expose fixed-parameter Ed25519 generators reject custom sizes.
        }
        return generator.generateKeyPair()
    }

    private fun loadFromExportStore(alias: String): KeyPair? {
        try {
            val encoded = exportStore!!.load(alias) ?: return null
            val bundle = KeyExportBundle.decodeBase64(encoded)
            val passphrase = requirePassphrase()
            try {
                val data = DeterministicKeyExporter.importKeyPair(bundle, passphrase)
                return KeyPair(data.publicKey, data.privateKey)
            } finally {
                passphrase.fill('\u0000')
            }
        } catch (ex: KeyExportException) {
            throw KeyManagementException("Failed to load deterministic key export", ex)
        }
    }

    private fun persistKey(alias: String, keyPair: KeyPair) {
        if (exportStore == null) return
        val passphrase = requirePassphrase()
        try {
            val bundle = DeterministicKeyExporter.exportKeyPair(
                keyPair.private, keyPair.public, alias, passphrase
            )
            exportStore.store(alias, bundle.encodeBase64())
        } catch (ex: KeyExportException) {
            throw KeyManagementException("Failed to persist deterministic key export", ex)
        } finally {
            passphrase.fill('\u0000')
        }
    }

    private fun requirePassphrase(): CharArray {
        if (passphraseProvider == null) {
            throw KeyManagementException("Passphrase provider must be configured for export store")
        }
        val passphrase = passphraseProvider.passphrase()
        if (passphrase == null || passphrase.isEmpty()) {
            throw KeyManagementException("Passphrase must not be empty")
        }
        return passphrase
    }

    companion object {
        @JvmStatic
        fun tryBouncyCastleGenerator(): KeyPairGenerator? {
            return try {
                val providerClass =
                    Class.forName("org.bouncycastle.jce.provider.BouncyCastleProvider")
                val provider =
                    providerClass.getDeclaredConstructor().newInstance() as Provider
                val providerName = provider.name
                if (Security.getProvider(providerName) == null) {
                    Security.addProvider(provider)
                }
                try {
                    KeyPairGenerator.getInstance("EdDSA", providerName)
                } catch (_: NoSuchAlgorithmException) {
                    KeyPairGenerator.getInstance("EdDSA")
                } catch (_: NoSuchProviderException) {
                    KeyPairGenerator.getInstance("EdDSA")
                }
            } catch (_: ClassNotFoundException) {
                null
            } catch (_: ReflectiveOperationException) {
                null
            } catch (_: ClassCastException) {
                null
            } catch (_: NoSuchAlgorithmException) {
                null
            }
        }

        private fun isExportable(keyPair: KeyPair?): Boolean {
            if (keyPair?.private == null || keyPair.public == null) return false
            val encoded = keyPair.private.encoded
            return encoded != null && encoded.isNotEmpty()
        }
    }
}
