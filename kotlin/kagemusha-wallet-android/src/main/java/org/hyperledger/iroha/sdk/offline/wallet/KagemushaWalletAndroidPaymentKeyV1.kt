// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.wallet

import android.content.pm.ApplicationInfo
import android.security.keystore.KeyProperties
import java.security.InvalidKeyException
import java.security.MessageDigest
import java.security.PrivateKey
import java.security.SignatureException
import java.security.cert.X509Certificate
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AndroidKeyAttestationOriginalV1
import org.hyperledger.iroha.sdk.offline.KagemushaP256Codec

/**
 * Android payment key of one wallet slot (spec §§2.2, 2.3, 4.2; G2 design rev 2 E3).
 *
 * The Keystore alias is `kgm-w1-<slot hex>`. The Rust provider draws every slot fresh and makes
 * the slot's intent durable before it asks for a key, so the alias is recorded before any key
 * exists under it; [intentGate] re-confirms that immediately before generation. Generating under
 * an existing alias would replace its key (AOSP keystore2 `rebind_alias`), so generation runs
 * only after a definitive `getKey == null`, never twice for one alias in this process, and never
 * after any alias entry was seen. Every Keystore error is `Unavailable`, never absence; nothing
 * here deletes or regenerates a key in response to an error.
 */
internal class KagemushaWalletAndroidPaymentKeyV1(
    private val keyStore: KagemushaWalletAndroidKeyStoreV1,
    private val environment: KagemushaWalletAndroidEnvironmentV1,
    private val intentGate: KagemushaWalletAndroidIntentGateV1,
) {
    private val lock = Any()

    /** Aliases this process generated under, saw occupied or deleted; never generated under again. */
    private val usedAliases = HashSet<String>()

    /** Tri-state probe through `KeyStore.getKey`; the public key comes from the entry's certificate. */
    fun probe(slot: ByteArray): KagemushaWalletAndroidKeyProbeV1 = synchronized(lock) {
        when (val probed = probeEntry(kagemushaWalletAndroidAliasV1(slot))) {
            is Probed.Present -> KagemushaWalletAndroidKeyProbeV1.Present(probed.publicKeySec1)
            Probed.Absent -> KagemushaWalletAndroidKeyProbeV1.Absent
            is Probed.Unavailable -> KagemushaWalletAndroidKeyProbeV1.Unavailable(probed.reason)
        }
    }

    /**
     * Generate the payment key of [slot] under [challengeDigest] and [profile] (design E3).
     *
     * Order: backup guard, in-process alias reuse, durable intent, definitive absence, hardware
     * plan, generation, then readback. A StrongBox failure falls back to the TEE only under
     * [KagemushaWalletAndroidKeyProfileV1.SECURE_ELEMENT_OR_TEE] and only after a second
     * definitive absence. A generated key that does not match the request is reported
     * `KEY_UNUSABLE` and left in place: the provider then abandons the slot.
     */
    fun generate(
        slot: ByteArray,
        challengeDigest: ByteArray,
        profile: KagemushaWalletAndroidKeyProfileV1,
    ): KagemushaWalletAndroidKeyGenerationV1 = synchronized(lock) { generateLocked(slot, challengeDigest, profile) }

    private fun generateLocked(
        slot: ByteArray,
        challengeDigest: ByteArray,
        profile: KagemushaWalletAndroidKeyProfileV1,
    ): KagemushaWalletAndroidKeyGenerationV1 {
        val alias = kagemushaWalletAndroidAliasV1(slot)
        val challenge = challengeDigest.copyOf()
        require(challenge.size == 32 && challenge.any { it != 0.toByte() }) {
            "attestation challenge must be the nonzero 32-byte challenge digest"
        }
        val refusal = try {
            kagemushaWalletAndroidBackupRefusalV1(environment)
        } catch (error: Throwable) {
            "backup configuration unreadable: ${error.javaClass.name}"
        }
        if (refusal != null) {
            return unavailableGeneration(KagemushaWalletAndroidUnavailableV1.platform(
                KagemushaWalletAndroidUnavailableV1.PLATFORM_BACKUP_ENABLED,
            ))
        }
        if (alias in usedAliases) return KagemushaWalletAndroidKeyGenerationV1.AlreadyPresent
        val intent = try {
            intentGate.intentState(slot.copyOf())
        } catch (error: Throwable) {
            KagemushaWalletAndroidIntentStateV1.UNAVAILABLE
        }
        when (intent) {
            KagemushaWalletAndroidIntentStateV1.DURABLE -> {}
            KagemushaWalletAndroidIntentStateV1.ABSENT -> return unavailableGeneration(
                KagemushaWalletAndroidUnavailableV1.platform(KagemushaWalletAndroidUnavailableV1.PLATFORM_INTENT_NOT_DURABLE),
            )
            KagemushaWalletAndroidIntentStateV1.UNAVAILABLE -> return unavailableGeneration(
                KagemushaWalletAndroidUnavailableV1.platform(KagemushaWalletAndroidUnavailableV1.PLATFORM_INTENT_UNAVAILABLE),
            )
        }
        when (val probed = probeEntry(alias)) {
            Probed.Absent -> {}
            is Probed.Present -> return occupied(alias)
            is Probed.Unavailable -> return unavailableGeneration(probed.reason)
        }
        val plan = try {
            kagemushaWalletAndroidHardwarePlanV1(profile, environment.apiLevel, environment.hasStrongBox())
        } catch (error: Throwable) {
            return unavailableGeneration(classify(error, KagemushaWalletAndroidUnavailableV1.PLATFORM_KEYSTORE))
        }
        val level = when (plan) {
            KagemushaWalletAndroidHardwarePlanV1.REFUSE -> return unavailableGeneration(
                KagemushaWalletAndroidUnavailableV1.platform(KagemushaWalletAndroidUnavailableV1.PLATFORM_STRONGBOX_UNAVAILABLE),
            )
            KagemushaWalletAndroidHardwarePlanV1.TEE_ONLY -> when (val tee = attempt(alias, challenge, strongBox = false)) {
                Attempt.Generated -> KagemushaWalletAndroidSecurityLevelV1.TRUSTED_ENVIRONMENT
                Attempt.StrongBoxUnavailable -> return unavailableGeneration(
                    KagemushaWalletAndroidUnavailableV1.platform(KagemushaWalletAndroidUnavailableV1.PLATFORM_GENERATION_FAILED),
                )
                is Attempt.Failed -> return unavailableGeneration(tee.reason)
            }
            KagemushaWalletAndroidHardwarePlanV1.STRONGBOX_ONLY,
            KagemushaWalletAndroidHardwarePlanV1.STRONGBOX_THEN_TEE -> when (val strong = attempt(alias, challenge, strongBox = true)) {
                Attempt.Generated -> KagemushaWalletAndroidSecurityLevelV1.STRONGBOX
                is Attempt.Failed -> return unavailableGeneration(strong.reason)
                Attempt.StrongBoxUnavailable -> {
                    if (plan != KagemushaWalletAndroidHardwarePlanV1.STRONGBOX_THEN_TEE) {
                        return unavailableGeneration(KagemushaWalletAndroidUnavailableV1.platform(
                            KagemushaWalletAndroidUnavailableV1.PLATFORM_STRONGBOX_UNAVAILABLE,
                        ))
                    }
                    // The TEE is permitted only while the alias is still definitively empty.
                    when (val again = probeEntry(alias)) {
                        Probed.Absent -> {}
                        is Probed.Present -> return occupied(alias)
                        is Probed.Unavailable -> return unavailableGeneration(again.reason)
                    }
                    when (val tee = attempt(alias, challenge, strongBox = false)) {
                        Attempt.Generated -> KagemushaWalletAndroidSecurityLevelV1.TRUSTED_ENVIRONMENT
                        Attempt.StrongBoxUnavailable -> return unavailableGeneration(
                            KagemushaWalletAndroidUnavailableV1.platform(KagemushaWalletAndroidUnavailableV1.PLATFORM_GENERATION_FAILED),
                        )
                        is Attempt.Failed -> return unavailableGeneration(tee.reason)
                    }
                }
            }
        }
        return readBack(alias, challenge, level)
    }

    /** Sign the exact [preimage] with `SHA256withECDSA`; the platform's DER is returned unmodified. */
    fun sign(slot: ByteArray, preimage: ByteArray): KagemushaWalletAndroidSignatureV1 =
        synchronized(lock) { signLocked(slot, preimage) }

    private fun signLocked(slot: ByteArray, preimage: ByteArray): KagemushaWalletAndroidSignatureV1 {
        val alias = kagemushaWalletAndroidAliasV1(slot)
        val message = preimage.copyOf()
        require(message.size in 1..KAGEMUSHA_WALLET_ANDROID_PREIMAGE_MAX_BYTES_V1) { "signing preimage is empty or oversized" }
        val key = when (val loaded = loadKey(alias)) {
            is Loaded.Key -> loaded.key
            Loaded.Absent -> return KagemushaWalletAndroidSignatureV1.Unavailable(
                KagemushaWalletAndroidUnavailableV1.platform(KagemushaWalletAndroidUnavailableV1.PLATFORM_KEY_ABSENT),
            )
            is Loaded.Unavailable -> return KagemushaWalletAndroidSignatureV1.Unavailable(loaded.reason)
        }
        return try {
            KagemushaWalletAndroidSignatureV1.Der(keyStore.sign(key, message))
        } catch (error: Throwable) {
            KagemushaWalletAndroidSignatureV1.Unavailable(
                when {
                    permanentlyInvalidated(error) -> KagemushaWalletAndroidUnavailableV1.PERMANENTLY_INVALIDATED
                    error is InvalidKeyException || error is SignatureException -> KagemushaWalletAndroidUnavailableV1.KEY_UNUSABLE
                    else -> KagemushaWalletAndroidUnavailableV1.platform(KagemushaWalletAndroidUnavailableV1.PLATFORM_SIGN_FAILED)
                },
            )
        }
    }

    /** Delete the key of [slot] (custody deletion step D3 only); the outcome comes from a fresh probe. */
    fun delete(slot: ByteArray): KagemushaWalletAndroidRemoveV1 = synchronized(lock) { deleteLocked(slot) }

    private fun deleteLocked(slot: ByteArray): KagemushaWalletAndroidRemoveV1 {
        val alias = kagemushaWalletAndroidAliasV1(slot)
        usedAliases += alias
        val failure = try {
            keyStore.deleteEntry(alias)
            null
        } catch (error: Throwable) {
            error
        }
        return when (val after = loadKey(alias)) {
            Loaded.Absent -> KagemushaWalletAndroidRemoveV1.Removed
            is Loaded.Key -> KagemushaWalletAndroidRemoveV1.NotRemoved(
                failure?.let { classify(it, KagemushaWalletAndroidUnavailableV1.PLATFORM_KEYSTORE) }
                    ?: KagemushaWalletAndroidUnavailableV1.platform(KagemushaWalletAndroidUnavailableV1.PLATFORM_KEYSTORE),
            )
            is Loaded.Unavailable -> KagemushaWalletAndroidRemoveV1.Uncertain(after.reason)
        }
    }

    /** Export the attestation chain (leaf first) of a present key; never absence from a null chain. */
    fun attestationChain(slot: ByteArray): KagemushaWalletAndroidAttestationChainV1 =
        synchronized(lock) { attestationChainLocked(slot) }

    private fun attestationChainLocked(slot: ByteArray): KagemushaWalletAndroidAttestationChainV1 {
        val alias = kagemushaWalletAndroidAliasV1(slot)
        val present = when (val probed = probeEntry(alias)) {
            is Probed.Present -> probed
            Probed.Absent -> return KagemushaWalletAndroidAttestationChainV1.Absent
            is Probed.Unavailable -> return KagemushaWalletAndroidAttestationChainV1.Unavailable(probed.reason)
        }
        return when (val chain = readChain(alias, present.publicKeySec1)) {
            is Chain.Valid -> KagemushaWalletAndroidAttestationChainV1.Present(chain.der)
            is Chain.Invalid -> KagemushaWalletAndroidAttestationChainV1.Unavailable(chain.reason)
        }
    }

    /**
     * One generation call. A failure leaves the alias usable only because the next generation
     * first needs a definitive absence again; a success marks the alias used for this process.
     */
    private fun attempt(alias: String, challenge: ByteArray, strongBox: Boolean): Attempt = try {
        keyStore.generate(KagemushaWalletAndroidKeySpecV1(alias, challenge, strongBox))
        usedAliases += alias
        Attempt.Generated
    } catch (unavailable: KagemushaWalletAndroidStrongBoxUnavailableV1) {
        Attempt.StrongBoxUnavailable
    } catch (error: Throwable) {
        Attempt.Failed(classify(error, KagemushaWalletAndroidUnavailableV1.PLATFORM_GENERATION_FAILED))
    }

    private fun readBack(
        alias: String,
        challenge: ByteArray,
        level: KagemushaWalletAndroidSecurityLevelV1,
    ): KagemushaWalletAndroidKeyGenerationV1 {
        val present = when (val probed = probeEntry(alias)) {
            is Probed.Present -> probed
            Probed.Absent -> return unavailableGeneration(
                KagemushaWalletAndroidUnavailableV1.platform(KagemushaWalletAndroidUnavailableV1.PLATFORM_GENERATION_FAILED),
            )
            is Probed.Unavailable -> return unavailableGeneration(probed.reason)
        }
        val facts = try {
            keyStore.facts(present.key)
        } catch (error: Throwable) {
            return unavailableGeneration(classify(error, KagemushaWalletAndroidUnavailableV1.PLATFORM_KEYSTORE))
        }
        if (!kagemushaWalletAndroidFactsMatchV1(facts, level, exportable = present.key.encoded != null)) {
            return unavailableGeneration(KagemushaWalletAndroidUnavailableV1.KEY_UNUSABLE)
        }
        val chain = when (val read = readChain(alias, present.publicKeySec1)) {
            is Chain.Valid -> read.certificates
            is Chain.Invalid -> return unavailableGeneration(read.reason)
        }
        val attested = try {
            AndroidKeyAttestationOriginalV1.challenge(chain)
        } catch (error: Throwable) {
            return unavailableGeneration(KagemushaWalletAndroidUnavailableV1.KEY_UNUSABLE)
        }
        if (!MessageDigest.isEqual(attested, challenge)) {
            return unavailableGeneration(KagemushaWalletAndroidUnavailableV1.KEY_UNUSABLE)
        }
        return KagemushaWalletAndroidKeyGenerationV1.Generated(present.publicKeySec1, level)
    }

    private fun readChain(alias: String, publicKeySec1: ByteArray): Chain {
        val certificates = try {
            keyStore.getCertificateChain(alias)
        } catch (error: Throwable) {
            return Chain.Invalid(classify(error, KagemushaWalletAndroidUnavailableV1.PLATFORM_CERTIFICATE))
        } ?: return Chain.Invalid(
            KagemushaWalletAndroidUnavailableV1.platform(KagemushaWalletAndroidUnavailableV1.PLATFORM_CERTIFICATE),
        )
        val x509 = certificates.map {
            it as? X509Certificate ?: return Chain.Invalid(
                KagemushaWalletAndroidUnavailableV1.platform(KagemushaWalletAndroidUnavailableV1.PLATFORM_CERTIFICATE),
            )
        }
        val der = try {
            x509.map { it.encoded }
        } catch (error: Throwable) {
            return Chain.Invalid(KagemushaWalletAndroidUnavailableV1.platform(KagemushaWalletAndroidUnavailableV1.PLATFORM_CERTIFICATE))
        }
        if (der.size !in 2..KAGEMUSHA_WALLET_ANDROID_CHAIN_MAX_CERTIFICATES_V1 ||
            der.any { it.size !in 1..KAGEMUSHA_WALLET_ANDROID_CERTIFICATE_MAX_BYTES_V1 }
        ) {
            return Chain.Invalid(KagemushaWalletAndroidUnavailableV1.platform(KagemushaWalletAndroidUnavailableV1.PLATFORM_CERTIFICATE))
        }
        val leaf = try {
            KagemushaP256Codec.uncompressedFromPublicKey(x509.first().publicKey)
        } catch (error: Throwable) {
            return Chain.Invalid(KagemushaWalletAndroidUnavailableV1.KEY_UNUSABLE)
        }
        if (!MessageDigest.isEqual(leaf, publicKeySec1)) return Chain.Invalid(KagemushaWalletAndroidUnavailableV1.KEY_UNUSABLE)
        return Chain.Valid(x509, der)
    }

    private fun probeEntry(alias: String): Probed {
        val key = when (val loaded = loadKey(alias)) {
            is Loaded.Key -> loaded.key
            Loaded.Absent -> return Probed.Absent
            is Loaded.Unavailable -> return Probed.Unavailable(loaded.reason)
        }
        val certificate = try {
            keyStore.getCertificate(alias)
        } catch (error: Throwable) {
            return Probed.Unavailable(classify(error, KagemushaWalletAndroidUnavailableV1.PLATFORM_CERTIFICATE))
        } ?: return Probed.Unavailable(
            KagemushaWalletAndroidUnavailableV1.platform(KagemushaWalletAndroidUnavailableV1.PLATFORM_CERTIFICATE),
        )
        val point = try {
            KagemushaP256Codec.uncompressedFromPublicKey(certificate.publicKey)
        } catch (error: Throwable) {
            return Probed.Unavailable(KagemushaWalletAndroidUnavailableV1.KEY_UNUSABLE)
        }
        return Probed.Present(key, point)
    }

    /** `getKey` only: null is absence, any throw is unavailable, a non-private key is unusable. */
    private fun loadKey(alias: String): Loaded {
        val key = try {
            keyStore.getKey(alias)
        } catch (error: Throwable) {
            return Loaded.Unavailable(classify(error, KagemushaWalletAndroidUnavailableV1.PLATFORM_KEYSTORE))
        } ?: return Loaded.Absent
        val privateKey = key as? PrivateKey ?: return Loaded.Unavailable(KagemushaWalletAndroidUnavailableV1.KEY_UNUSABLE)
        return Loaded.Key(privateKey)
    }

    private fun occupied(alias: String): KagemushaWalletAndroidKeyGenerationV1 {
        usedAliases += alias
        return KagemushaWalletAndroidKeyGenerationV1.AlreadyPresent
    }

    private fun unavailableGeneration(reason: KagemushaWalletAndroidUnavailableV1) =
        KagemushaWalletAndroidKeyGenerationV1.Unavailable(reason)

    private fun permanentlyInvalidated(error: Throwable): Boolean = try {
        keyStore.isPermanentlyInvalidated(error)
    } catch (classification: Throwable) {
        false
    }

    private fun classify(error: Throwable, platformCode: Int): KagemushaWalletAndroidUnavailableV1 =
        if (permanentlyInvalidated(error)) {
            KagemushaWalletAndroidUnavailableV1.PERMANENTLY_INVALIDATED
        } else {
            KagemushaWalletAndroidUnavailableV1.platform(platformCode)
        }

    private sealed class Loaded {
        class Key(val key: PrivateKey) : Loaded()
        object Absent : Loaded()
        class Unavailable(val reason: KagemushaWalletAndroidUnavailableV1) : Loaded()
    }

    private sealed class Probed {
        class Present(val key: PrivateKey, val publicKeySec1: ByteArray) : Probed()
        object Absent : Probed()
        class Unavailable(val reason: KagemushaWalletAndroidUnavailableV1) : Probed()
    }

    private sealed class Attempt {
        object Generated : Attempt()
        object StrongBoxUnavailable : Attempt()
        class Failed(val reason: KagemushaWalletAndroidUnavailableV1) : Attempt()
    }

    private sealed class Chain {
        class Valid(val certificates: List<X509Certificate>, val der: List<ByteArray>) : Chain()
        class Invalid(val reason: KagemushaWalletAndroidUnavailableV1) : Chain()
    }
}

/** Largest preimage the payment key signs; provider preimages are role-tagged bodies of a few KiB. */
internal const val KAGEMUSHA_WALLET_ANDROID_PREIMAGE_MAX_BYTES_V1: Int = 1 shl 20

/** Attestation chain bounds, as in `AndroidKeyAttestationOriginalV1`. */
internal const val KAGEMUSHA_WALLET_ANDROID_CHAIN_MAX_CERTIFICATES_V1: Int = 8
internal const val KAGEMUSHA_WALLET_ANDROID_CERTIFICATE_MAX_BYTES_V1: Int = 16 * 1024

/** Keystore alias of [slot]: `kgm-w1-` and 64 lowercase hex digits. The slot must be nonzero. */
internal fun kagemushaWalletAndroidAliasV1(slot: ByteArray): String {
    require(slot.size == 32 && slot.any { it != 0.toByte() }) { "wallet slot must be 32 nonzero bytes" }
    val hex = StringBuilder(KAGEMUSHA_WALLET_ANDROID_ALIAS_PREFIX_V1.length + 64)
    hex.append(KAGEMUSHA_WALLET_ANDROID_ALIAS_PREFIX_V1)
    for (byte in slot) {
        val value = byte.toInt() and 0xff
        hex.append(HEX_DIGITS[value ushr 4]).append(HEX_DIGITS[value and 0x0f])
    }
    return hex.toString()
}

private const val HEX_DIGITS = "0123456789abcdef"

/** Hardware generation plan of one profile on one device. */
internal enum class KagemushaWalletAndroidHardwarePlanV1 { STRONGBOX_ONLY, STRONGBOX_THEN_TEE, TEE_ONLY, REFUSE }

/** StrongBox when the device has it; the TEE only when the profile permits it. */
internal fun kagemushaWalletAndroidHardwarePlanV1(
    profile: KagemushaWalletAndroidKeyProfileV1,
    apiLevel: Int,
    hasStrongBox: Boolean,
): KagemushaWalletAndroidHardwarePlanV1 {
    val strongBox = apiLevel >= 28 && hasStrongBox
    return when (profile) {
        KagemushaWalletAndroidKeyProfileV1.SECURE_ELEMENT ->
            if (strongBox) KagemushaWalletAndroidHardwarePlanV1.STRONGBOX_ONLY else KagemushaWalletAndroidHardwarePlanV1.REFUSE
        KagemushaWalletAndroidKeyProfileV1.SECURE_ELEMENT_OR_TEE ->
            if (strongBox) KagemushaWalletAndroidHardwarePlanV1.STRONGBOX_THEN_TEE else KagemushaWalletAndroidHardwarePlanV1.TEE_ONLY
    }
}

/**
 * Whether a generated key's `KeyInfo` matches the request: secure hardware at the planned level
 * (readable on API 31+), generated on the device, `PURPOSE_SIGN` and `SHA-256` only, P-256, not
 * exportable, no user authentication, presence or confirmation, and unlimited use on API 31+.
 */
internal fun kagemushaWalletAndroidFactsMatchV1(
    facts: KagemushaWalletAndroidKeyFactsV1,
    level: KagemushaWalletAndroidSecurityLevelV1,
    exportable: Boolean,
): Boolean {
    val modern = facts.apiLevel >= 31
    val expectedLevel = when (level) {
        KagemushaWalletAndroidSecurityLevelV1.STRONGBOX -> KeyProperties.SECURITY_LEVEL_STRONGBOX
        KagemushaWalletAndroidSecurityLevelV1.TRUSTED_ENVIRONMENT -> KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT
    }
    val modernReadback = if (modern) {
        facts.securityLevel == expectedLevel && facts.remainingUsageCount == KeyProperties.UNRESTRICTED_USAGE_COUNT
    } else {
        facts.securityLevel == null && facts.remainingUsageCount == null
    }
    return modernReadback &&
        facts.insideSecureHardware &&
        facts.origin == KeyProperties.ORIGIN_GENERATED &&
        facts.purposes == KeyProperties.PURPOSE_SIGN &&
        facts.digests == setOf(KeyProperties.DIGEST_SHA256) &&
        facts.keySize == 256 &&
        !facts.userAuthenticationRequired &&
        !facts.userPresenceRequired &&
        !facts.userConfirmationRequired &&
        !exportable
}

/**
 * Why this application may not hold wallet custody, or null when it may: the backup set must be
 * empty, so `android:allowBackup` must be false and the app may not declare its own backup agent
 * (G2 design rev 2: a full-data restore stream that reaches an app without its own agent clears
 * its data, including `no_backup`, and its Keystore namespace).
 */
internal fun kagemushaWalletAndroidBackupRefusalV1(environment: KagemushaWalletAndroidEnvironmentV1): String? = when {
    environment.applicationFlags() and ApplicationInfo.FLAG_ALLOW_BACKUP != 0 ->
        "KAGEMUSHA wallet custody requires android:allowBackup=\"false\"; do not override the library manifest"
    environment.backupAgentName() != null ->
        "KAGEMUSHA wallet custody forbids an application backup agent"
    else -> null
}
