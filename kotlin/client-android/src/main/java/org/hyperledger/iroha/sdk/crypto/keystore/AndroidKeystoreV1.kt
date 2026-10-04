// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.crypto.keystore

import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyInfo
import android.security.keystore.KeyProperties
import android.security.keystore.StrongBoxUnavailableException
import androidx.annotation.Keep
import androidx.annotation.RequiresApi
import java.security.Key
import java.security.KeyFactory
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.PrivateKey
import java.security.PublicKey
import java.security.Signature
import java.security.cert.Certificate
import java.security.spec.ECGenParameterSpec

/** Lowest API level with keystore2, whose `KeyStore.getKey` distinguishes "no key" from an error. */
internal const val ANDROID_KEYSTORE2_MIN_API_V1: Int = 31

/** The AndroidKeyStore could not say whether an alias holds a key. Never read as absence. */
internal class AndroidKeystoreUnavailableExceptionV1(message: String, cause: Throwable? = null) :
    IllegalStateException(message, cause)

/** A definitive answer of [probeAndroidKeystoreAliasV1]; an unknown answer is a throw, never a value. */
internal enum class AndroidKeystoreAliasStateV1 { PRESENT, ABSENT }

/**
 * Tri-state AndroidKeyStore alias probe (AOSP android14 `AndroidKeyStoreSpi`).
 *
 * `containsAlias` is `getKeyMetadata(alias) != null`, and `getKeyMetadata` returns null for every
 * Keystore error; `getEntry`, `aliases`, `isKeyEntry` and `getCertificate*` mask errors the same
 * way. Only `KeyStore.getKey(alias, null)` on keystore2 (API 31+) is tri-state: a key is present,
 * null is `KEY_NOT_FOUND`, and any throw means the Keystore did not answer. On keystore1
 * (API < 31) `getKey` returns null whenever its `KeyStore.contains` probe fails, so a null there is
 * never definitive; a returned key is.
 *
 * Generating under an occupied alias replaces its key (keystore2 `rebind_alias`; keystore1 deletes
 * every entry of the alias first), so callers generate only after this probe returned null.
 *
 * @return the present key, or null for a definitive absence
 * @throws AndroidKeystoreUnavailableExceptionV1 when [getKey] throws, or below API 31 when it
 *   reports no key
 */
internal fun probeAndroidKeystoreAliasV1(apiLevel: Int, alias: String, getKey: (String) -> Key?): Key? {
    val key = try {
        getKey(alias)
    } catch (error: Exception) {
        throw AndroidKeystoreUnavailableExceptionV1("AndroidKeyStore could not answer for the alias", error)
    }
    if (key == null && apiLevel < ANDROID_KEYSTORE2_MIN_API_V1) {
        throw AndroidKeystoreUnavailableExceptionV1("keystore1 (API $apiLevel) cannot prove that an alias is empty")
    }
    return key
}

/**
 * Read-only AndroidKeyStore entry access. Errors are thrown, never turned into absence; existence
 * is answered only by [getKey] through [probe].
 */
internal interface AndroidKeystoreEntriesV1 {
    /** Platform API level; [getKey] is tri-state only from [ANDROID_KEYSTORE2_MIN_API_V1]. */
    val apiLevel: Int

    /** `KeyStore.getKey(alias, null)`: a key, null for no key entry, or a throw. */
    fun getKey(alias: String): Key?

    /** Certificate chain (leaf first) of an entry [getKey] reported present; null is an error. */
    fun getCertificateChain(alias: String): List<Certificate>?
}

/**
 * Exact AndroidKeyStore EC P-256 signing-key request: `PURPOSE_SIGN`, `DIGEST_SHA256` only, an
 * attestation challenge, StrongBox when [strongBox], and a usage limit only when [maxUsageCount]
 * is set.
 */
internal class AndroidKeystoreEcKeyRequestV1(
    val alias: String,
    challenge: ByteArray,
    val strongBox: Boolean,
    val maxUsageCount: Int? = null,
) {
    private val attestationChallenge = challenge.copyOf()

    init {
        require(alias.isNotBlank()) { "alias must not be blank" }
        require(attestationChallenge.isNotEmpty()) { "attestation challenge must not be empty" }
        require(maxUsageCount == null || maxUsageCount > 0) { "usage limit must be positive" }
    }

    fun challenge(): ByteArray = attestationChallenge.copyOf()
}

/** Thrown by [AndroidKeystoreV1.generate] for a platform `StrongBoxUnavailableException`. */
internal class AndroidKeystoreStrongBoxUnavailableV1(cause: Throwable?) :
    Exception("StrongBox is unavailable", cause)

/** API 31+ `KeyInfo` readback of a present key, independent of Android types. */
internal class AndroidKeystoreKeyFactsV1(
    val insideSecureHardware: Boolean,
    /** `KeyInfo.getSecurityLevel`. */
    val securityLevel: Int,
    /** `KeyInfo.getRemainingUsageCount`. */
    val remainingUsageCount: Int,
    val origin: Int,
    val purposes: Int,
    digests: Set<String>,
) {
    val digests: Set<String> = digests.toSet()
}

/**
 * Narrow AndroidKeyStore access for KAGEMUSHA app keys and one-use diagnostics.
 *
 * Every method reports errors by throwing. The masking `KeyStore` calls (`containsAlias`,
 * `getEntry`, `aliases`, `isKeyEntry`, `getCertificate`) are not offered.
 */
internal interface AndroidKeystoreV1 : AndroidKeystoreEntriesV1 {
    /**
     * Generate under [request] and return the generated public key. Throws
     * [AndroidKeystoreStrongBoxUnavailableV1] when StrongBox is unavailable. Any key already under
     * the alias is replaced, so call this only after [probe] returned null.
     */
    fun generate(request: AndroidKeystoreEcKeyRequestV1): PublicKey

    /** API 31+ `KeyInfo` readback of a present key. */
    fun facts(key: PrivateKey): AndroidKeystoreKeyFactsV1

    /** `SHA256withECDSA` over [message] with a present key, as the platform's DER. */
    fun sign(key: PrivateKey, message: ByteArray): ByteArray

    /** `KeyStore.deleteEntry(alias)`: keystore2 treats a missing alias as deleted and throws for any other error. */
    fun deleteEntry(alias: String)
}

/** Tri-state probe of [alias]; see [probeAndroidKeystoreAliasV1]. */
internal fun AndroidKeystoreEntriesV1.probe(alias: String): Key? =
    probeAndroidKeystoreAliasV1(apiLevel, alias) { getKey(it) }

/** [probe] as a definitive [AndroidKeystoreAliasStateV1]; an unknown answer throws. */
internal fun AndroidKeystoreEntriesV1.aliasState(alias: String): AndroidKeystoreAliasStateV1 =
    if (probe(alias) == null) AndroidKeystoreAliasStateV1.ABSENT else AndroidKeystoreAliasStateV1.PRESENT

/** Production [AndroidKeystoreV1] over the platform `AndroidKeyStore`. */
internal class AndroidSystemKeystoreV1 : AndroidKeystoreV1 {
    override val apiLevel: Int get() = Build.VERSION.SDK_INT

    private fun keyStore(): KeyStore = KeyStore.getInstance(ANDROID_KEYSTORE_PROVIDER_V1).apply { load(null) }

    override fun getKey(alias: String): Key? = keyStore().getKey(alias, null)

    override fun getCertificateChain(alias: String): List<Certificate>? =
        keyStore().getCertificateChain(alias)?.toList()

    override fun generate(request: AndroidKeystoreEcKeyRequestV1): PublicKey {
        check(apiLevel >= ANDROID_KEYSTORE2_MIN_API_V1) { "AndroidKeyStore generation requires keystore2 (API 31+)" }
        return AndroidKeystoreApi31V1.generate(request)
    }

    override fun facts(key: PrivateKey): AndroidKeystoreKeyFactsV1 {
        check(apiLevel >= ANDROID_KEYSTORE2_MIN_API_V1) { "KeyInfo readback requires keystore2 (API 31+)" }
        val info = KeyFactory.getInstance(key.algorithm, ANDROID_KEYSTORE_PROVIDER_V1).getKeySpec(key, KeyInfo::class.java)
        return AndroidKeystoreApi31V1.facts(info)
    }

    override fun sign(key: PrivateKey, message: ByteArray): ByteArray =
        Signature.getInstance("SHA256withECDSA").run {
            initSign(key)
            update(message)
            sign()
        }

    override fun deleteEntry(alias: String) {
        keyStore().deleteEntry(alias)
    }
}

private const val ANDROID_KEYSTORE_PROVIDER_V1 = "AndroidKeyStore"

/** Keeps API 28/31 method and exception linkage out of classes loaded on older devices. */
@RequiresApi(31)
@Keep
private object AndroidKeystoreApi31V1 {
    fun generate(request: AndroidKeystoreEcKeyRequestV1): PublicKey {
        val builder = KeyGenParameterSpec.Builder(request.alias, KeyProperties.PURPOSE_SIGN)
            .setAlgorithmParameterSpec(ECGenParameterSpec("secp256r1"))
            .setDigests(KeyProperties.DIGEST_SHA256)
            .setAttestationChallenge(request.challenge())
            .setIsStrongBoxBacked(request.strongBox)
        request.maxUsageCount?.let { builder.setMaxUsageCount(it) }
        val generator = KeyPairGenerator.getInstance(KeyProperties.KEY_ALGORITHM_EC, ANDROID_KEYSTORE_PROVIDER_V1)
        return try {
            generator.initialize(builder.build())
            generator.generateKeyPair().public
        } catch (unavailable: StrongBoxUnavailableException) {
            throw AndroidKeystoreStrongBoxUnavailableV1(unavailable)
        }
    }

    // `isInsideSecureHardware` is deprecated in favour of `securityLevel`; both are read so the
    // readback cross-checks two platform answers.
    @Suppress("DEPRECATION")
    fun facts(info: KeyInfo): AndroidKeystoreKeyFactsV1 = AndroidKeystoreKeyFactsV1(
        insideSecureHardware = info.isInsideSecureHardware,
        securityLevel = info.securityLevel,
        remainingUsageCount = info.remainingUsageCount,
        origin = info.origin,
        purposes = info.purposes,
        digests = info.digests.toSet(),
    )
}
