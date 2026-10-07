// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.crypto.keystore

import android.os.Build
import android.security.keystore.KeyInfo
import android.security.keystore.KeyProperties
import java.security.Key
import java.security.KeyFactory
import java.security.PrivateKey
import java.security.Signature
import java.security.cert.Certificate
import java.security.cert.X509Certificate
import org.hyperledger.iroha.sdk.auth.FirstDeviceAuthProtocolV1 as Protocol
import org.hyperledger.iroha.sdk.auth.FirstDeviceAuthRegistrationBridgeV1 as RegistrationBridge
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AndroidKeyAttestationOriginalV1 as Original
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AttestationResult

/** Existing authentication-purpose P-256 AndroidKeyStore custody, separate from Wallet payment keys.
 *
 * There is deliberately no key-generation, deletion, fallback, software-key or public freshness API.
 * A separate private Native original auth-intent owner is required before initial generation can be
 * implemented. Neither challenge data nor a missing alias grants that authority. This component
 * reads actual KeyInfo and original attestation metadata; Core separately verifies roots/revocation,
 * package policy, Google authorization and application Integrity. It creates no verified result.
 * Calls are synchronous platform work; the app must run them off its UI thread, after Google UI.
 */
object FirstDeviceAuthAndroidKeyV1 {
    sealed interface Lookup {
        class Present internal constructor(val key: ExistingKey) : Lookup
        /** Definitive keystore2 absence only, without authority to generate. */
        data object Absent : Lookup
        /** API26–30 null remains unknown, and every provider error remains unavailable. */
        class Unavailable internal constructor(val reason: Reason) : Lookup
    }

    enum class Reason { UNSUPPORTED_API, KEY_ABSENCE_UNKNOWN, KEYSTORE_UNAVAILABLE, KEY_UNUSABLE }

    /** Read-only access to a separate `iroha-auth-a1-` alias; payment/Ed25519 aliases cannot enter. */
    @JvmStatic
    fun openExisting(slot: ByteArray, challenge: Protocol.Challenge, requireOriginalOwner: () -> Unit): Lookup =
        openExistingWithAccess(slot, challenge, requireOriginalOwner, AndroidFirstDeviceAuthKeyAccessV1())

    internal fun openExistingWithAccess(slot: ByteArray, challenge: Protocol.Challenge,
        requireOriginalOwner: () -> Unit, access: FirstDeviceAuthKeyAccessV1): Lookup {
        val alias = firstDeviceAuthAliasV1(slot)
        requireOriginalOwner()
        if (access.apiLevel < 26) return Lookup.Unavailable(Reason.UNSUPPORTED_API)
        val key = try { access.entries.probe(alias).also { requireOriginalOwner() } }
        catch (unavailable: AndroidKeystoreUnavailableExceptionV1) {
            requireOriginalOwner()
            return Lookup.Unavailable(if (access.apiLevel < 31 && unavailable.cause == null)
                Reason.KEY_ABSENCE_UNKNOWN else Reason.KEYSTORE_UNAVAILABLE)
        }
        if (key == null) return Lookup.Absent.also { requireOriginalOwner() }
        val observed = try { readExisting(alias, key, challenge, access).also { requireOriginalOwner() } }
        catch (invalid: Exception) { requireOriginalOwner(); return Lookup.Unavailable(Reason.KEY_UNUSABLE) }
        return Lookup.Present(ExistingKey.fromOriginal(alias, challenge, observed, access, requireOriginalOwner))
            .also { requireOriginalOwner() }
    }

    /** Original alias-held private key. It is not a Native verified result, credential or generation grant. */
    class ExistingKey private constructor(
        private val alias: String,
        private val challenge: Protocol.Challenge,
        private val original: FirstDeviceAuthObservedKeyV1,
        private val access: FirstDeviceAuthKeyAccessV1,
        private val requireOriginalOwner: () -> Unit,
    ) {
        fun publicKeySec1Bytes(): ByteArray { recheck(); return original.publicKey.copyOf().also { recheck() } }

        /** Exact complete leaf-first chain; no certificate is cropped, rebuilt or replaced. */
        fun certificateChainDerBytes(): List<ByteArray> {
            recheck()
            return original.der.map { it.copyOf() }.also { recheck() }
        }

        /** SHA256/P-256 signs the entire authentication possession original, never payment's 32 bytes. */
        fun signPossessionOriginal(raw: Protocol.RawOriginal): ByteArray {
            recheck()
            Protocol.requireRawBinding(challenge, raw)
            Protocol.requireRawChainBinding(raw, original.der)
            require(raw.appPublicKeySec1Bytes().contentEquals(original.publicKey)) {
                "Authentication verifier public key differs from original alias"
            }
            val message = Protocol.possessionMessageBytes(challenge, raw)
            recheck()
            val signature = try { access.sign(original.privateKey, message.copyOf()) }
            catch (failure: Exception) { requireOriginalOwner(); throw failure }
            requireOriginalOwner()
            val der = Protocol.canonicalPossessionDerBytes(signature)
            verifyFirstDeviceAuthPossessionDerV1(original.leaf, message, der)
            recheck()
            return der.copyOf().also { recheck() }
        }

        /** The separate closed registration purpose; never an arbitrary message signing API.
         * The app rechecks the existing Ed registration/runtime and its durable exact frame record.
         * Core independently verifies this whole-frame signature before its existing owner CAS.
         */
        fun signRegistrationBridgeOriginal(frame: RegistrationBridge.FrameOriginal,
            requireOriginalRegistration: () -> Unit): ByteArray {
            fun guard() { requireOriginalRegistration(); recheck(); requireOriginalRegistration() }
            guard()
            frame.requireKeyBinding(challenge, original.publicKey, original.der)
            val message = frame.originalBytes()
            guard()
            val signature = try { access.sign(original.privateKey, message.copyOf()) }
            catch (failure: Exception) { requireOriginalOwner(); requireOriginalRegistration(); throw failure }
            guard()
            val der = Protocol.canonicalPossessionDerBytes(signature)
            verifyFirstDeviceAuthPossessionDerV1(original.leaf, message, der)
            guard()
            return der.copyOf().also { guard() }
        }

        /** Positive lookup and all retained originals are rechecked for every use; no cached readiness. */
        fun recheck() {
            requireOriginalOwner()
            val key = access.entries.probe(alias) ?: error("Original authentication key is unavailable")
            val observed = readExisting(alias, key, challenge, access)
            require(observed.publicKey.contentEquals(original.publicKey) &&
                observed.level == original.level && observed.der.size == original.der.size &&
                observed.der.zip(original.der).all { (a, b) -> a.contentEquals(b) }) {
                "Original authentication key or attestation chain changed"
            }
            requireOriginalOwner()
        }

        internal companion object {
            internal fun fromOriginal(alias: String, challenge: Protocol.Challenge,
                observed: FirstDeviceAuthObservedKeyV1, access: FirstDeviceAuthKeyAccessV1,
                requireOriginalOwner: () -> Unit): ExistingKey =
                ExistingKey(alias, challenge, observed, access, requireOriginalOwner)
        }
    }

    private fun readExisting(alias: String, key: Key, challenge: Protocol.Challenge,
        access: FirstDeviceAuthKeyAccessV1): FirstDeviceAuthObservedKeyV1 {
        val privateKey = key as? PrivateKey ?: error("Authentication alias is not a private key")
        require(privateKey.algorithm == "EC" && privateKey.encoded == null) {
            "Authentication key must be nonexportable EC"
        }
        val certificates = checkNotNull(access.entries.getCertificateChain(alias)).map {
            it as? X509Certificate ?: error("Authentication certificate is not X509")
        }
        val der = certificates.map { it.encoded.copyOf() }
        require(der.size in 2..8 && der.all { it.size in 1..Protocol.MAX_CERTIFICATE_BYTES })
        val level = Original.persistentAppHardwareSecurityLevel(certificates, challenge.attestationChallengeBytes())
        requireFirstDeviceAuthFactsV1(access.facts(privateKey), access.apiLevel, level)
        val point = Original.publicKeySec1(certificates)
        return FirstDeviceAuthObservedKeyV1(privateKey, certificates.first(), der, point, level)
    }
}

internal fun firstDeviceAuthAliasV1(slot: ByteArray): String {
    val original = slot.copyOf()
    require(original.size == 32 && original.any { it != 0.toByte() })
    return "iroha-auth-a1-" + original.joinToString("") { "%02x".format(it.toInt() and 255) }
}

internal class FirstDeviceAuthObservedKeyV1(val privateKey: PrivateKey, val leaf: X509Certificate,
    der: List<ByteArray>, publicKey: ByteArray, val level: AttestationResult.SecurityLevel) {
    val der = der.map { it.copyOf() }
    val publicKey = publicKey.copyOf()
}

internal class FirstDeviceAuthKeyFactsV1(val insideHardware: Boolean, val securityLevel: Int?,
    val remainingUsageCount: Int?, val origin: Int, val purposes: Int, digests: Set<String>,
    val keySize: Int, val userAuthenticationRequired: Boolean,
    val userPresenceRequired: Boolean?, val userConfirmationRequired: Boolean?) {
    val digests = digests.toSet()
}

internal fun requireFirstDeviceAuthFactsV1(facts: FirstDeviceAuthKeyFactsV1, apiLevel: Int,
    level: AttestationResult.SecurityLevel) {
    require(apiLevel >= 26 && facts.insideHardware && facts.origin == KeyProperties.ORIGIN_GENERATED &&
        facts.purposes == KeyProperties.PURPOSE_SIGN && facts.digests == setOf(KeyProperties.DIGEST_SHA256) &&
        facts.keySize == 256 && !facts.userAuthenticationRequired)
    require(level == AttestationResult.SecurityLevel.TRUSTED_ENVIRONMENT || level == AttestationResult.SecurityLevel.STRONG_BOX)
    if (apiLevel < 28) require(level == AttestationResult.SecurityLevel.TRUSTED_ENVIRONMENT)
    if (apiLevel >= 28) require(facts.userPresenceRequired == false && facts.userConfirmationRequired == false)
    else require(facts.userPresenceRequired == null && facts.userConfirmationRequired == null)
    if (apiLevel >= 31) {
        require(facts.remainingUsageCount == KeyProperties.UNRESTRICTED_USAGE_COUNT &&
            facts.securityLevel == if (level == AttestationResult.SecurityLevel.STRONG_BOX)
                KeyProperties.SECURITY_LEVEL_STRONGBOX else KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT)
    } else require(facts.securityLevel == null && facts.remainingUsageCount == null)
}

internal fun verifyFirstDeviceAuthPossessionDerV1(leaf: X509Certificate, message: ByteArray, der: ByteArray) {
    val originalMessage = message.copyOf()
    val originalDer = Protocol.canonicalPossessionDerBytes(der)
    require(Signature.getInstance("SHA256withECDSA").run {
        initVerify(leaf.publicKey); update(originalMessage); verify(originalDer)
    }) { "Authentication possession signature differs from full original" }
}

internal interface FirstDeviceAuthKeyAccessV1 {
    val apiLevel: Int
    val entries: AndroidKeystoreEntriesV1
    fun facts(key: PrivateKey): FirstDeviceAuthKeyFactsV1
    fun sign(key: PrivateKey, message: ByteArray): ByteArray
}

internal class AndroidFirstDeviceAuthKeyAccessV1 : FirstDeviceAuthKeyAccessV1 {
    override val apiLevel: Int get() = Build.VERSION.SDK_INT
    override val entries: AndroidKeystoreEntriesV1 = AndroidSystemKeystoreV1()
    @Suppress("DEPRECATION")
    override fun facts(key: PrivateKey): FirstDeviceAuthKeyFactsV1 {
        val info = KeyFactory.getInstance(key.algorithm, "AndroidKeyStore").getKeySpec(key, KeyInfo::class.java)
        return FirstDeviceAuthKeyFactsV1(info.isInsideSecureHardware,
            if (apiLevel >= 31) FirstDeviceAuthKeyInfoApi31V1.securityLevel(info) else null,
            if (apiLevel >= 31) FirstDeviceAuthKeyInfoApi31V1.remainingUsageCount(info) else null,
            info.origin, info.purposes, info.digests.toSet(), info.keySize, info.isUserAuthenticationRequired,
            if (apiLevel >= 28) FirstDeviceAuthKeyInfoApi28V1.presence(info) else null,
            if (apiLevel >= 28) FirstDeviceAuthKeyInfoApi28V1.confirmation(info) else null)
    }
    override fun sign(key: PrivateKey, message: ByteArray): ByteArray = Signature.getInstance("SHA256withECDSA").run {
        initSign(key); update(message.copyOf()); sign()
    }
}

private object FirstDeviceAuthKeyInfoApi28V1 {
    fun presence(info: KeyInfo): Boolean = info.isTrustedUserPresenceRequired
    fun confirmation(info: KeyInfo): Boolean = info.isUserConfirmationRequired
}

private object FirstDeviceAuthKeyInfoApi31V1 {
    fun securityLevel(info: KeyInfo): Int = info.securityLevel
    fun remainingUsageCount(info: KeyInfo): Int = info.remainingUsageCount
}
