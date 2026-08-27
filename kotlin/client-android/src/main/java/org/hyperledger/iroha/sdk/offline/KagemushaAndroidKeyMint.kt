// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import android.annotation.SuppressLint
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyInfo
import android.security.keystore.KeyProperties
import java.io.ByteArrayOutputStream
import java.io.IOException
import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.security.GeneralSecurityException
import java.security.KeyFactory
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.MessageDigest
import java.security.ProviderException
import java.security.PublicKey
import java.security.Signature
import java.security.cert.CertificateEncodingException
import java.security.interfaces.ECPublicKey
import java.security.spec.ECGenParameterSpec
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Physical Android KeyMint path for one-use Kagemusha request authorization.
 *
 * This service is deliberately separate from the generic transaction keystore. It requires
 * Android 12 / API 31 and [PackageManager.FEATURE_KEYSTORE_SINGLE_USE_KEY], because Core accepts an
 * Android registration only when a usage count of one is attested in hardware. The generated key
 * has the exact `EC/secp256r1`, sign-only, SHA-256, maximum-one-use profile.
 *
 * [StrongBoxPolicy.REQUIRED] never falls back to a weaker TEE key. [authorize] consumes the key,
 * validates the platform DER signature through the shared Kagemusha codec, finalizes the native
 * authorization, and removes the exhausted alias.
 */
class KagemushaAndroidKeyMint private constructor(
    private val backend: Backend,
) {
    private val owner = Any()

    /** Create the physical KeyMint service for the current Android application. */
    @Throws(GeneralSecurityException::class)
    constructor(context: Context) : this(PlatformBackend(context.applicationContext))

    /** Closed StrongBox policy: either do not request it, or require it without downgrade. */
    enum class StrongBoxPolicy {
        NOT_REQUESTED,
        REQUIRED,
    }

    /**
     * Derive the exact pre-key challenge, create the physical assertion key, and construct the
     * matching on-chain registration.
     */
    @Throws(GeneralSecurityException::class)
    fun generateRegistration(
        alias: String,
        parameters: RegistrationParameters,
        strongBoxPolicy: StrongBoxPolicy,
    ): GeneratedRegistration {
        val material = generateRegistrationMaterial(
            alias,
            parameters.attestationChallenge(),
            strongBoxPolicy,
        )
        return try {
            GeneratedRegistration(parameters.registration(material), material)
        } catch (failure: RuntimeException) {
            cleanupRegistrationMaterial(material, failure)
            throw failure
        } catch (failure: Error) {
            cleanupRegistrationMaterial(material, failure)
            throw failure
        }
    }

    /**
     * Generate one hardware-enforced, single-use assertion key.
     *
     * The challenge must come from
     * [DeviceAttestationRegistration.androidPreKeyGenerationChallengeHash]. Applications which do
     * not need a split flow should use [generateRegistration].
     */
    @Throws(GeneralSecurityException::class)
    fun generateRegistrationMaterial(
        alias: String,
        attestationChallenge: ByteArray,
        strongBoxPolicy: StrongBoxPolicy,
    ): RegistrationMaterial {
        val canonicalAlias = requireAlias(alias)
        val challenge = requireChallenge(attestationChallenge)
        requirePlatformCapabilities(strongBoxPolicy)

        val request = GenerationRequest(
            canonicalAlias,
            challenge,
            strongBoxPolicy == StrongBoxPolicy.REQUIRED,
        )
        val generated = backend.generate(request)
        return try {
            if (!generated.insideSecureHardware()) {
                throw GeneralSecurityException(
                    "Kagemusha KeyMint assertion key is not inside secure hardware",
                )
            }
            if (generated.remainingUsageCount() != MAX_USAGE_COUNT) {
                throw GeneralSecurityException(
                    "Kagemusha KeyMint assertion key does not expose one remaining hardware use",
                )
            }
            if (strongBoxPolicy == StrongBoxPolicy.REQUIRED && !generated.strongBoxBacked()) {
                throw GeneralSecurityException(
                    "StrongBox was required but KeyMint generated a weaker key",
                )
            }

            val publicKey = KagemushaP256Codec.requireUncompressedPublicKey(
                generated.publicKeySec1(),
            )
            val certificateChain = requireCertificateChain(generated.certificateChainDer())
            RegistrationMaterial(
                owner,
                canonicalAlias,
                publicKey,
                certificateChain,
                encodeCertificateArray(certificateChain),
                generated.strongBoxBacked(),
            )
        } catch (failure: GeneralSecurityException) {
            deleteAfterRejectedGeneration(canonicalAlias, failure)
            throw failure
        } catch (failure: RuntimeException) {
            deleteAfterRejectedGeneration(canonicalAlias, failure)
            throw failure
        } catch (failure: Error) {
            deleteAfterRejectedGeneration(canonicalAlias, failure)
            throw failure
        }
    }

    /** Consume one generated assertion key to authorize the exact native preparation. */
    @Throws(GeneralSecurityException::class)
    fun authorize(
        preparation: KagemushaRecursiveSpendProver.RequestAuthorizationPreparation,
        material: RegistrationMaterial,
    ): KagemushaRecursiveSpendProver.RequestAuthorization {
        val signatureDer = signPreparationForAuthorization(material, preparation.signingBytes())
        return KagemushaRecursiveSpendProver.finalizeRequestAuthorization(
            preparation,
            signatureDer,
        )
    }

    @Throws(GeneralSecurityException::class)
    internal fun signPreparationForAuthorization(
        material: RegistrationMaterial,
        signingBytes: ByteArray,
    ): ByteArray {
        val requiredMaterial = requireOwnedMaterial(material)
        val message = copyRequired(signingBytes, "signingBytes")
        check(requiredMaterial.consumed.compareAndSet(false, true)) {
            "Kagemusha KeyMint registration material is already consumed"
        }

        val signatureDer = try {
            backend.sign(requiredMaterial.aliasValue, SIGNATURE_ALGORITHM, message)
        } catch (failure: GeneralSecurityException) {
            deleteAfterRejectedGeneration(requiredMaterial.aliasValue, failure)
            throw failure
        } catch (failure: RuntimeException) {
            deleteAfterRejectedGeneration(requiredMaterial.aliasValue, failure)
            throw failure
        } catch (failure: Error) {
            deleteAfterRejectedGeneration(requiredMaterial.aliasValue, failure)
            throw failure
        }
        backend.delete(requiredMaterial.aliasValue)
        KagemushaP256Codec.rawLowSFromStrictDer(signatureDer)
        return signatureDer.copyOf()
    }

    /** Delete an unused generated alias and permanently consume its material. */
    @Throws(GeneralSecurityException::class)
    fun delete(material: RegistrationMaterial) {
        val requiredMaterial = requireOwnedMaterial(material)
        requiredMaterial.consumed.set(true)
        backend.delete(requiredMaterial.aliasValue)
    }

    @Throws(GeneralSecurityException::class)
    private fun requirePlatformCapabilities(policy: StrongBoxPolicy) {
        if (backend.apiLevel() < MINIMUM_API_LEVEL) {
            throw GeneralSecurityException(
                "Kagemusha KeyMint single-use assertions require Android 12 / API 31",
            )
        }
        if (!backend.supportsHardwareSingleUse()) {
            throw GeneralSecurityException(
                "device lacks hardware-enforced AndroidKeyStore single-use keys",
            )
        }
        if (policy == StrongBoxPolicy.REQUIRED && !backend.supportsStrongBox()) {
            throw GeneralSecurityException(
                "StrongBox is required by policy but unavailable on this device",
            )
        }
    }

    private fun requireOwnedMaterial(material: RegistrationMaterial): RegistrationMaterial {
        require(material.ownerToken === owner) {
            "registration material belongs to a different Kagemusha KeyMint service"
        }
        return material
    }

    private fun cleanupRegistrationMaterial(material: RegistrationMaterial, failure: Throwable) {
        try {
            delete(material)
        } catch (cleanupFailure: GeneralSecurityException) {
            failure.addSuppressed(cleanupFailure)
        }
    }

    private fun deleteAfterRejectedGeneration(alias: String, failure: Throwable) {
        try {
            backend.delete(alias)
        } catch (cleanupFailure: GeneralSecurityException) {
            failure.addSuppressed(cleanupFailure)
        }
    }

    /** Immutable material required to construct an Android device-attestation registration. */
    class RegistrationMaterial internal constructor(
        internal val ownerToken: Any,
        internal val aliasValue: String,
        publicKeySec1: ByteArray,
        certificateChainDer: List<ByteArray>,
        attestationReport: ByteArray,
        private val strongBoxBackedValue: Boolean,
    ) {
        private val publicKeySec1Value = publicKeySec1.copyOf()
        private val certificateChainDerValue = certificateChainDer.map { it.copyOf() }
        private val attestationReportValue = attestationReport.copyOf()
        internal val consumed = AtomicBoolean()

        fun alias(): String = aliasValue

        /** Lowercase SHA-256 of [assertionPublicKeySec1], as required by registration. */
        fun keyId(): String = keyId(publicKeySec1Value)

        fun assertionPublicKeySec1(): ByteArray = publicKeySec1Value.copyOf()

        /** Leaf-first Android KeyMint X.509 chain, defensively copied. */
        fun certificateChainDer(): List<ByteArray> =
            certificateChainDerValue.map { it.copyOf() }

        /** Canonical definite-length CBOR certificate array for `attestation_report`. */
        fun attestationReport(): ByteArray = attestationReportValue.copyOf()

        fun strongBoxBacked(): Boolean = strongBoxBackedValue

        fun isConsumed(): Boolean = consumed.get()
    }

    /** Exact fields which exist before Android KeyMint creates the assertion key. */
    class RegistrationParameters(
        private val deviceId: String,
        private val accountId: String,
        private val assetDefinitionId: String?,
        private val androidPackageName: String,
        androidSigningCertificateSha256: ByteArray,
        private val deviceAuthorityPublicKey: KagemushaDevicePublicKeyV2,
        private val recentBlockHeight: Long,
        recentBlockHash: ByteArray,
        private val expiresAtMs: Long,
    ) {
        private val androidSigningCertificateSha256Value =
            androidSigningCertificateSha256.copyOf()
        private val recentBlockHashValue = recentBlockHash.copyOf()
        private val attestationChallengeValue =
            DeviceAttestationRegistration.androidPreKeyGenerationChallengeHash(
                DeviceAttestationRegistration.REGISTRATION_VERSION,
                deviceId,
                accountId,
                assetDefinitionId,
                androidPackageName,
                androidSigningCertificateSha256Value,
                deviceAuthorityPublicKey,
                recentBlockHeight,
                recentBlockHashValue,
                expiresAtMs,
            )

        fun attestationChallenge(): ByteArray = attestationChallengeValue.copyOf()

        internal fun registration(material: RegistrationMaterial): DeviceAttestationRegistration =
            DeviceAttestationRegistration(
                version = DeviceAttestationRegistration.REGISTRATION_VERSION,
                platform = DeviceAttestationRegistration.ANDROID_KEYMINT_PLATFORM,
                keyId = material.keyId(),
                deviceId = deviceId,
                accountId = accountId,
                assetDefinitionId = assetDefinitionId,
                iosTeamId = null,
                iosBundleId = null,
                iosEnvironment = null,
                androidPackageName = androidPackageName,
                androidSigningCertificateSha256 = androidSigningCertificateSha256Value,
                publicKey = deviceAuthorityPublicKey,
                assertionScheme = DeviceAttestationRegistration.ANDROID_KEYMINT_ASSERTION_SCHEME,
                assertionKeyAlgorithm =
                    DeviceAttestationRegistration.ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM,
                assertionPublicKey = material.assertionPublicKeySec1(),
                assertionUsageCountLimit = MAX_USAGE_COUNT,
                oneUse = true,
                challengeHash = attestationChallengeValue,
                attestationReportHash = null,
                attestationReport = material.attestationReport(),
                evidenceHash = null,
                evidence = null,
                recentBlockHeight = recentBlockHeight,
                recentBlockHash = recentBlockHashValue,
                expiresAtMs = expiresAtMs,
            )
    }

    /** Registration plus the retained one-use key handle needed for online authorization. */
    class GeneratedRegistration internal constructor(
        private val registrationValue: DeviceAttestationRegistration,
        private val materialValue: RegistrationMaterial,
    ) {
        fun registration(): DeviceAttestationRegistration = registrationValue

        fun material(): RegistrationMaterial = materialValue
    }

    internal class GenerationRequest(
        private val aliasValue: String,
        challenge: ByteArray,
        private val strongBoxRequiredValue: Boolean,
    ) {
        private val challengeValue = challenge.copyOf()

        fun alias(): String = aliasValue

        fun challenge(): ByteArray = challengeValue.copyOf()

        fun strongBoxRequired(): Boolean = strongBoxRequiredValue

        fun keyAlgorithm(): String = KEY_ALGORITHM

        fun curveName(): String = CURVE_NAME

        fun purposes(): Int = PURPOSES

        fun digest(): String = DIGEST

        fun maxUsageCount(): Int = MAX_USAGE_COUNT
    }

    internal class GeneratedKey(
        publicKeySec1: ByteArray,
        certificateChainDer: List<ByteArray>,
        private val insideSecureHardwareValue: Boolean,
        private val strongBoxBackedValue: Boolean,
        private val remainingUsageCountValue: Int,
    ) {
        private val publicKeySec1Value = publicKeySec1.copyOf()
        private val certificateChainDerValue = certificateChainDer.map { it.copyOf() }

        fun publicKeySec1(): ByteArray = publicKeySec1Value.copyOf()

        fun certificateChainDer(): List<ByteArray> =
            certificateChainDerValue.map { it.copyOf() }

        fun insideSecureHardware(): Boolean = insideSecureHardwareValue

        fun strongBoxBacked(): Boolean = strongBoxBackedValue

        fun remainingUsageCount(): Int = remainingUsageCountValue
    }

    internal interface Backend {
        fun apiLevel(): Int

        fun supportsHardwareSingleUse(): Boolean

        fun supportsStrongBox(): Boolean

        @Throws(GeneralSecurityException::class)
        fun generate(request: GenerationRequest): GeneratedKey

        @Throws(GeneralSecurityException::class)
        fun sign(alias: String, algorithm: String, message: ByteArray): ByteArray

        @Throws(GeneralSecurityException::class)
        fun delete(alias: String)
    }

    @SuppressLint("NewApi")
    @Suppress("DEPRECATION")
    private class PlatformBackend(private val context: Context) : Backend {
        init {
            loadKeyStore()
        }

        override fun apiLevel(): Int = Build.VERSION.SDK_INT

        override fun supportsHardwareSingleUse(): Boolean =
            context.packageManager.hasSystemFeature(PackageManager.FEATURE_KEYSTORE_SINGLE_USE_KEY)

        override fun supportsStrongBox(): Boolean =
            context.packageManager.hasSystemFeature(PackageManager.FEATURE_STRONGBOX_KEYSTORE)

        @Throws(GeneralSecurityException::class)
        override fun generate(request: GenerationRequest): GeneratedKey {
            val keyStore = loadKeyStore()
            if (keyStore.containsAlias(request.alias())) {
                throw GeneralSecurityException(
                    "AndroidKeyStore alias already exists: ${request.alias()}",
                )
            }
            try {
                val builder = KeyGenParameterSpec.Builder(request.alias(), PURPOSES)
                    .setAlgorithmParameterSpec(ECGenParameterSpec(CURVE_NAME))
                    .setDigests(DIGEST)
                    .setAttestationChallenge(request.challenge())
                    .setMaxUsageCount(MAX_USAGE_COUNT)
                if (request.strongBoxRequired()) {
                    builder.setIsStrongBoxBacked(true)
                }

                val generator = KeyPairGenerator.getInstance(KEY_ALGORITHM, ANDROID_KEYSTORE)
                generator.initialize(builder.build())
                val keyPair = generator.generateKeyPair()
                val privateKey = keyPair.private
                val keyFactory = KeyFactory.getInstance(privateKey.algorithm, ANDROID_KEYSTORE)
                val keyInfo = keyFactory.getKeySpec(privateKey, KeyInfo::class.java)
                val strongBox =
                    keyInfo.securityLevel == KeyProperties.SECURITY_LEVEL_STRONGBOX
                val acceptedSecurityLevel = strongBox ||
                    keyInfo.securityLevel == KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT
                if (!keyInfo.isInsideSecureHardware || !acceptedSecurityLevel) {
                    throw GeneralSecurityException(
                        "AndroidKeyStore generated a software-backed Kagemusha assertion key",
                    )
                }
                if (
                    keyInfo.purposes != PURPOSES ||
                    keyInfo.keySize != 256 ||
                    keyInfo.remainingUsageCount != MAX_USAGE_COUNT ||
                    !keyInfo.digests.contentEquals(arrayOf(DIGEST))
                ) {
                    throw GeneralSecurityException(
                        "AndroidKeyStore generated a key outside the Kagemusha KeyMint profile",
                    )
                }

                val chain = keyStore.getCertificateChain(request.alias())
                if (chain == null || chain.isEmpty()) {
                    throw GeneralSecurityException(
                        "AndroidKeyStore did not return a KeyMint attestation chain",
                    )
                }
                val certificateChain = chain.map { certificate -> certificate.encoded }
                val generatedPublicKey = uncompressedSec1(keyPair.public)
                val attestedPublicKey = uncompressedSec1(chain[0].publicKey)
                if (!MessageDigest.isEqual(generatedPublicKey, attestedPublicKey)) {
                    throw GeneralSecurityException(
                        "KeyMint attestation leaf does not bind the generated assertion key",
                    )
                }
                return GeneratedKey(
                    generatedPublicKey,
                    certificateChain,
                    insideSecureHardwareValue = true,
                    strongBoxBackedValue = strongBox,
                    remainingUsageCountValue = keyInfo.remainingUsageCount,
                )
            } catch (failure: CertificateEncodingException) {
                val wrapped = GeneralSecurityException(
                    "Android KeyMint key generation failed",
                    failure,
                )
                deleteWithSuppressed(request.alias(), wrapped)
                throw wrapped
            } catch (failure: ProviderException) {
                val wrapped = GeneralSecurityException(
                    "Android KeyMint key generation failed",
                    failure,
                )
                deleteWithSuppressed(request.alias(), wrapped)
                throw wrapped
            } catch (failure: GeneralSecurityException) {
                deleteWithSuppressed(request.alias(), failure)
                throw failure
            } catch (failure: RuntimeException) {
                deleteWithSuppressed(request.alias(), failure)
                throw failure
            } catch (failure: Error) {
                deleteWithSuppressed(request.alias(), failure)
                throw failure
            }
        }

        @Throws(GeneralSecurityException::class)
        override fun sign(alias: String, algorithm: String, message: ByteArray): ByteArray {
            val entry = loadKeyStore().getEntry(alias, null)
            if (entry !is KeyStore.PrivateKeyEntry) {
                throw GeneralSecurityException(
                    "Kagemusha KeyMint assertion alias is unavailable",
                )
            }
            return Signature.getInstance(algorithm).run {
                initSign(entry.privateKey)
                update(message)
                sign()
            }
        }

        @Throws(GeneralSecurityException::class)
        override fun delete(alias: String) {
            val keyStore = loadKeyStore()
            if (keyStore.containsAlias(alias)) {
                keyStore.deleteEntry(alias)
            }
        }

        @Throws(GeneralSecurityException::class)
        private fun loadKeyStore(): KeyStore {
            try {
                return KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }
            } catch (failure: IOException) {
                throw GeneralSecurityException("failed to load AndroidKeyStore", failure)
            }
        }

        private fun deleteWithSuppressed(alias: String, failure: Throwable) {
            try {
                delete(alias)
            } catch (cleanupFailure: GeneralSecurityException) {
                failure.addSuppressed(cleanupFailure)
            }
        }
    }

    companion object {
        @JvmField val MINIMUM_API_LEVEL: Int = Build.VERSION_CODES.S
        @JvmField val KEY_ALGORITHM: String = KeyProperties.KEY_ALGORITHM_EC
        const val CURVE_NAME: String = "secp256r1"
        @JvmField val DIGEST: String = KeyProperties.DIGEST_SHA256
        const val SIGNATURE_ALGORITHM: String = "SHA256withECDSA"
        @JvmField val PURPOSES: Int = KeyProperties.PURPOSE_SIGN
        const val MAX_USAGE_COUNT: Int = 1

        private const val ANDROID_KEYSTORE: String = "AndroidKeyStore"
        private const val MAX_ALIAS_BYTES: Int = 128
        private const val MAX_ATTESTATION_REPORT_BYTES: Int = 64 * 1024

        internal fun withBackendForTests(backend: Backend): KagemushaAndroidKeyMint =
            KagemushaAndroidKeyMint(backend)

        private fun requireAlias(alias: String): String {
            require(
                alias.isNotEmpty() &&
                    alias == alias.trim() &&
                    alias.toByteArray(StandardCharsets.UTF_8).size <= MAX_ALIAS_BYTES,
            ) { "alias must be canonical non-empty text within 128 UTF-8 bytes" }
            require(alias.none { it.code < 0x20 || it.code == 0x7f }) {
                "alias must not contain control characters"
            }
            return alias
        }

        private fun requireChallenge(challenge: ByteArray): ByteArray {
            val value = challenge.copyOf()
            require(value.size == 32 && (value[31].toInt() and 1) == 1) {
                "attestationChallenge must be a canonical 32-byte Iroha hash"
            }
            return value
        }

        private fun copyRequired(value: ByteArray, field: String): ByteArray {
            require(value.isNotEmpty()) { "$field must not be empty" }
            return value.copyOf()
        }

        @Throws(GeneralSecurityException::class)
        private fun requireCertificateChain(certificates: List<ByteArray>): List<ByteArray> {
            if (certificates.isEmpty()) {
                throw GeneralSecurityException(
                    "Android KeyMint did not return an attestation certificate chain",
                )
            }
            return certificates.map { certificate ->
                if (certificate.isEmpty()) {
                    throw GeneralSecurityException(
                        "Android KeyMint returned an empty attestation certificate",
                    )
                }
                certificate.copyOf()
            }
        }

        @Throws(GeneralSecurityException::class)
        private fun encodeCertificateArray(certificates: List<ByteArray>): ByteArray {
            val out = ByteArrayOutputStream()
            writeCborHead(out, 4, certificates.size)
            certificates.forEach { certificate ->
                writeCborHead(out, 2, certificate.size)
                out.write(certificate, 0, certificate.size)
            }
            return out.toByteArray().also { encoded ->
                if (encoded.size > MAX_ATTESTATION_REPORT_BYTES) {
                    throw GeneralSecurityException(
                        "Android KeyMint certificate array exceeds the registration report bound",
                    )
                }
            }
        }

        @Throws(GeneralSecurityException::class)
        private fun writeCborHead(out: ByteArrayOutputStream, major: Int, value: Int) {
            if (value < 0) {
                throw GeneralSecurityException("negative CBOR length")
            }
            when {
                value <= 23 -> out.write((major shl 5) or value)
                value <= 0xff -> {
                    out.write((major shl 5) or 24)
                    out.write(value)
                }
                value <= 0xffff -> {
                    out.write((major shl 5) or 25)
                    out.write((value ushr 8) and 0xff)
                    out.write(value and 0xff)
                }
                else -> {
                    out.write((major shl 5) or 26)
                    out.write((value ushr 24) and 0xff)
                    out.write((value ushr 16) and 0xff)
                    out.write((value ushr 8) and 0xff)
                    out.write(value and 0xff)
                }
            }
        }

        @Throws(GeneralSecurityException::class)
        private fun fixedUnsigned(value: BigInteger): ByteArray {
            val signed = value.toByteArray()
            val sourceOffset = if (signed.size == 33 && signed[0].toInt() == 0) 1 else 0
            val length = signed.size - sourceOffset
            if (length > 32) {
                throw GeneralSecurityException("P-256 coordinate exceeds 32 bytes")
            }
            return ByteArray(32).also { fixed ->
                System.arraycopy(signed, sourceOffset, fixed, fixed.size - length, length)
            }
        }

        @Throws(GeneralSecurityException::class)
        private fun uncompressedSec1(publicKey: PublicKey): ByteArray {
            val ecPublicKey = publicKey as? ECPublicKey
                ?: throw GeneralSecurityException(
                    "Android KeyMint did not generate an EC public key",
                )
            val x = fixedUnsigned(ecPublicKey.w.affineX)
            val y = fixedUnsigned(ecPublicKey.w.affineY)
            return ByteArray(65).also { encoded ->
                encoded[0] = 0x04
                System.arraycopy(x, 0, encoded, 1, x.size)
                System.arraycopy(y, 0, encoded, 33, y.size)
                KagemushaP256Codec.requireUncompressedPublicKey(encoded)
            }
        }

        private fun keyId(publicKey: ByteArray): String {
            val digest = try {
                MessageDigest.getInstance("SHA-256").digest(publicKey)
            } catch (failure: GeneralSecurityException) {
                throw IllegalStateException("SHA-256 unavailable", failure)
            }
            val alphabet = "0123456789abcdef"
            return buildString(digest.size * 2) {
                digest.forEach { value ->
                    val unsigned = value.toInt() and 0xff
                    append(alphabet[unsigned ushr 4])
                    append(alphabet[unsigned and 0x0f])
                }
            }
        }
    }
}
