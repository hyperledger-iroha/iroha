// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.wallet

import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyInfo
import android.security.keystore.KeyPermanentlyInvalidatedException
import android.security.keystore.KeyProperties
import android.security.keystore.StrongBoxUnavailableException
import java.security.Key
import java.security.KeyFactory
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.PrivateKey
import java.security.Signature
import java.security.cert.Certificate
import java.security.spec.ECGenParameterSpec

/**
 * Exact payment-key generation request (spec §§2.2, 2.3; G2 design rev 2 E3).
 *
 * Only [alias], [challengeDigest] and [strongBox] vary. The key is always EC secp256r1 with
 * `PURPOSE_SIGN` and `DIGEST_SHA256` only, and never bound to user authentication, an unlocked
 * device, a usage count, user presence or user confirmation: removing the screen lock would
 * otherwise invalidate the key and destroy the balance (spec §2.3).
 */
internal class KagemushaWalletAndroidKeySpecV1(
    val alias: String,
    challengeDigest: ByteArray,
    val strongBox: Boolean,
) {
    private val challenge = challengeDigest.copyOf()

    init {
        require(alias.startsWith(KAGEMUSHA_WALLET_ANDROID_ALIAS_PREFIX_V1)) { "payment-key alias is not a slot alias" }
        require(challenge.size == 32) { "attestation challenge must be the 32-byte challenge digest" }
    }

    /** The attestation challenge: the issuer challenge digest. */
    fun challengeDigest(): ByteArray = challenge.copyOf()

    val curve: String get() = "secp256r1"
    val purposes: Int get() = KeyProperties.PURPOSE_SIGN
    val digests: Set<String> get() = setOf(KeyProperties.DIGEST_SHA256)
    val userAuthenticationRequired: Boolean get() = false
    val unlockedDeviceRequired: Boolean get() = false
    val userPresenceRequired: Boolean get() = false
    val userConfirmationRequired: Boolean get() = false
    val maxUsageCount: Int? get() = null
}

/** Readback of a generated key's `KeyInfo`, independent of Android types. */
internal class KagemushaWalletAndroidKeyFactsV1(
    val apiLevel: Int,
    val insideSecureHardware: Boolean,
    /** `KeyInfo.getSecurityLevel` on API 31+, otherwise null. */
    val securityLevel: Int?,
    /** `KeyInfo.getRemainingUsageCount` on API 31+, otherwise null. */
    val remainingUsageCount: Int?,
    val origin: Int,
    val purposes: Int,
    digests: Set<String>,
    val keySize: Int,
    val userAuthenticationRequired: Boolean,
    val userPresenceRequired: Boolean,
    val userConfirmationRequired: Boolean,
) {
    val digests: Set<String> = digests.toSet()
}

/** Thrown by [KagemushaWalletAndroidKeyStoreV1.generate] for `StrongBoxUnavailableException`. */
internal class KagemushaWalletAndroidStrongBoxUnavailableV1(cause: Throwable?) :
    Exception("StrongBox is unavailable", cause)

/**
 * Narrow AndroidKeyStore access used by [KagemushaWalletAndroidPaymentKeyV1].
 *
 * Every method reports errors by throwing; none of them turns an error into absence. Only
 * [getKey] answers whether an entry exists. `containsAlias`, `aliases`, `size`, `isKeyEntry` and
 * `getCertificate*` swallow Keystore errors as "absent" (AOSP `AndroidKeyStoreSpi`), so they are
 * not offered here for existence decisions: [getCertificate] and [getCertificateChain] are read
 * only after [getKey] returned a key, and a null there is never absence.
 */
internal interface KagemushaWalletAndroidKeyStoreV1 {
    /** `KeyStore.getKey(alias, null)`: a key, null for no key entry, or a throw. */
    fun getKey(alias: String): Key?

    /** Certificate of an entry [getKey] reported present; null is an error, never absence. */
    fun getCertificate(alias: String): Certificate?

    /** Attestation chain of an entry [getKey] reported present; null is an error. */
    fun getCertificateChain(alias: String): List<Certificate>?

    /** Generate under [spec]; throws [KagemushaWalletAndroidStrongBoxUnavailableV1] when StrongBox is unavailable. */
    fun generate(spec: KagemushaWalletAndroidKeySpecV1)

    /** `KeyInfo` readback of a present key. */
    fun facts(key: PrivateKey): KagemushaWalletAndroidKeyFactsV1

    /** `SHA256withECDSA` over [preimage], returned as the platform's DER. */
    fun sign(key: PrivateKey, preimage: ByteArray): ByteArray

    /** `KeyStore.deleteEntry(alias)`. */
    fun deleteEntry(alias: String)

    /** Whether [error] or one of its causes reports a permanently invalidated key. */
    fun isPermanentlyInvalidated(error: Throwable): Boolean
}

/** Keystore alias prefix of payment keys: `kgm-w1-<64 lowercase hex slot digits>`. */
internal const val KAGEMUSHA_WALLET_ANDROID_ALIAS_PREFIX_V1: String = "kgm-w1-"

/** Production AndroidKeyStore access. */
internal class KagemushaWalletAndroidSystemKeyStoreV1 : KagemushaWalletAndroidKeyStoreV1 {
    private fun keyStore(): KeyStore = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }

    override fun getKey(alias: String): Key? = keyStore().getKey(alias, null)

    override fun getCertificate(alias: String): Certificate? = keyStore().getCertificate(alias)

    override fun getCertificateChain(alias: String): List<Certificate>? =
        keyStore().getCertificateChain(alias)?.toList()

    override fun generate(spec: KagemushaWalletAndroidKeySpecV1) {
        // Only the setters below are called. In particular there is no
        // user-authentication, unlocked-device, usage-count, user-presence or
        // user-confirmation setter (spec §2.3); the backup-rules test guards this file.
        val builder = KeyGenParameterSpec.Builder(spec.alias, spec.purposes)
            .setAlgorithmParameterSpec(ECGenParameterSpec(spec.curve))
            .setDigests(*spec.digests.toTypedArray())
            .setAttestationChallenge(spec.challengeDigest())
        val generator = KeyPairGenerator.getInstance(KeyProperties.KEY_ALGORITHM_EC, ANDROID_KEYSTORE)
        if (spec.strongBox) {
            check(Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) { "StrongBox needs API 28" }
            KagemushaWalletAndroidStrongBoxApi28V1.generate(builder, generator)
        } else {
            generator.initialize(builder.build())
            generator.generateKeyPair()
        }
    }

    @Suppress("DEPRECATION")
    override fun facts(key: PrivateKey): KagemushaWalletAndroidKeyFactsV1 {
        val info = KeyFactory.getInstance(key.algorithm, ANDROID_KEYSTORE).getKeySpec(key, KeyInfo::class.java)
        val api = Build.VERSION.SDK_INT
        val modern = api >= Build.VERSION_CODES.S
        val confirmations = api >= Build.VERSION_CODES.P
        return KagemushaWalletAndroidKeyFactsV1(
            apiLevel = api,
            insideSecureHardware = info.isInsideSecureHardware,
            securityLevel = if (modern) KagemushaWalletAndroidKeyInfoApi31V1.securityLevel(info) else null,
            remainingUsageCount = if (modern) KagemushaWalletAndroidKeyInfoApi31V1.remainingUsageCount(info) else null,
            origin = info.origin,
            purposes = info.purposes,
            digests = info.digests.toSet(),
            keySize = info.keySize,
            userAuthenticationRequired = info.isUserAuthenticationRequired,
            userPresenceRequired = confirmations && KagemushaWalletAndroidKeyInfoApi28V1.userPresenceRequired(info),
            userConfirmationRequired = confirmations && KagemushaWalletAndroidKeyInfoApi28V1.userConfirmationRequired(info),
        )
    }

    override fun sign(key: PrivateKey, preimage: ByteArray): ByteArray =
        Signature.getInstance(SIGNATURE_ALGORITHM).run {
            initSign(key)
            update(preimage)
            sign()
        }

    override fun deleteEntry(alias: String) {
        keyStore().deleteEntry(alias)
    }

    override fun isPermanentlyInvalidated(error: Throwable): Boolean =
        generateSequence(error) { it.cause }.take(8).any { it is KeyPermanentlyInvalidatedException }

    private companion object {
        const val ANDROID_KEYSTORE = "AndroidKeyStore"
        const val SIGNATURE_ALGORITHM = "SHA256withECDSA"
    }
}

/** Keeps API 28 method and exception linkage out of the API 24-27 TEE path. */
@android.annotation.TargetApi(28)
private object KagemushaWalletAndroidStrongBoxApi28V1 {
    fun generate(builder: KeyGenParameterSpec.Builder, generator: KeyPairGenerator) {
        try {
            generator.initialize(builder.setIsStrongBoxBacked(true).build())
            generator.generateKeyPair()
        } catch (unavailable: StrongBoxUnavailableException) {
            throw KagemushaWalletAndroidStrongBoxUnavailableV1(unavailable)
        }
    }
}

@android.annotation.TargetApi(28)
private object KagemushaWalletAndroidKeyInfoApi28V1 {
    fun userPresenceRequired(info: KeyInfo): Boolean = info.isTrustedUserPresenceRequired
    fun userConfirmationRequired(info: KeyInfo): Boolean = info.isUserConfirmationRequired
}

@android.annotation.TargetApi(31)
private object KagemushaWalletAndroidKeyInfoApi31V1 {
    fun securityLevel(info: KeyInfo): Int = info.securityLevel
    fun remainingUsageCount(info: KeyInfo): Int = info.remainingUsageCount
}
