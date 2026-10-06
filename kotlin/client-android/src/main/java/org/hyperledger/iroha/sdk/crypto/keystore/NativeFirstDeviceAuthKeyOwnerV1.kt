// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.crypto.keystore

import android.content.Context
import android.os.Build
import android.os.UserManager
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.security.keystore.StrongBoxUnavailableException
import java.security.KeyPairGenerator
import java.security.spec.ECGenParameterSpec
import java.security.cert.X509Certificate
import org.hyperledger.iroha.sdk.auth.FirstDeviceAuthProtocolV1 as Protocol
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AndroidKeyAttestationOriginalV1 as Original
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AttestationResult

/** Native durable owner for the separate authentication-purpose app key.
 *
 * Native chooses the slot, durably retains the full operation transcript and consumes a
 * private original one-shot grant before the private generation upcall. Existing intents
 * are recover-only, including when [reserveOriginal] is called again. Restore never creates
 * an intent or key. These methods perform synchronous filesystem/keystore work and must run
 * off the UI thread with the originating Google UI/runtime/request owner guard supplied.
 * Neither observed key data nor a parsed challenge establishes Core verification or Wallet
 * readiness. This owner does not touch account, payment, confirmation or recovery keys.
 */
object NativeFirstDeviceAuthKeyOwnerV1 {
    @JvmStatic
    fun reserveOriginal(context: Context, challenge: Protocol.Challenge,
        requireOriginalOwner: () -> Unit): OriginalKeyData {
        requireOriginalOwner()
        val platform = FirstDeviceAuthNativePlatformV1(context, challenge, requireOriginalOwner)
        return receive(platform, NativeFirstDeviceAuthKeyJniV1.reserve(platform, challenge.transcriptBytes()))
            .also { requireOriginalOwner() }
    }

    /** Read only: unavailable storage/key is retry, never fresh generation or repaired history. */
    @JvmStatic
    fun restoreOriginal(context: Context, challenge: Protocol.Challenge,
        requireOriginalOwner: () -> Unit): OriginalKeyData {
        requireOriginalOwner()
        val platform = FirstDeviceAuthNativePlatformV1(context, challenge, requireOriginalOwner)
        return receive(platform, NativeFirstDeviceAuthKeyJniV1.restore(platform, challenge.transcriptBytes()))
            .also { requireOriginalOwner() }
    }

    private fun receive(platform: FirstDeviceAuthNativePlatformV1,
        reply: NativeFirstDeviceAuthKeyReplyV1?): OriginalKeyData {
        platform.requireOriginal()
        val value = checkNotNull(reply) { "Native authentication key owner is unavailable" }
        check(value.status == 0) { "Authentication key intent is recover-only or unavailable (${value.status})" }
        return OriginalKeyData.fromOriginal(platform, value.slotBytes(), value.publicKeyBytes())
    }

    /** Original observed key data, with no public constructor or mutable grant/freshness fields. */
    class OriginalKeyData private constructor(private val platform: FirstDeviceAuthNativePlatformV1,
        slot: ByteArray, publicKey: ByteArray) {
        private val slot = slot.copyOf()
        private val publicKey = publicKey.copyOf()

        fun slotBytes(): ByteArray = slot.copyOf().also { recheck() }
        fun publicKeySec1Bytes(): ByteArray = publicKey.copyOf().also { recheck() }

        /** The actual read-only SDK reader rechecks KeyInfo and complete original attestation. */
        fun openExisting(): FirstDeviceAuthAndroidKeyV1.ExistingKey {
            recheck()
            val result = FirstDeviceAuthAndroidKeyV1.openExisting(slot, platform.challenge) { recheck() }
            val existing = (result as? FirstDeviceAuthAndroidKeyV1.Lookup.Present)?.key
                ?: error("Original authentication key is unavailable; retry without regeneration")
            check(existing.publicKeySec1Bytes().contentEquals(publicKey)) {
                "Original authentication public point changed"
            }
            recheck()
            return existing
        }

        private fun recheck() {
            platform.requireOriginal()
            val reply = checkNotNull(NativeFirstDeviceAuthKeyJniV1.restore(platform, platform.challenge.transcriptBytes()))
            check(reply.status == 0 && reply.slotBytes().contentEquals(slot) &&
                reply.publicKeyBytes().contentEquals(publicKey)) { "Native authentication intent changed or is unavailable" }
            platform.requireOriginal()
        }

        internal companion object {
            internal fun fromOriginal(platform: FirstDeviceAuthNativePlatformV1, slot: ByteArray,
                publicKey: ByteArray): OriginalKeyData {
                require(slot.size == 32 && slot.any { it != 0.toByte() })
                require(publicKey.size == 65 && publicKey[0] == 4.toByte())
                return OriginalKeyData(platform, slot, publicKey)
            }
        }
    }
}

/** Private JNI platform. The callback is not an app-facing key-generation API. */
internal class FirstDeviceAuthNativePlatformV1(context: Context, val challenge: Protocol.Challenge,
    private val requireOriginalOwner: () -> Unit) {
    private val context = context.applicationContext ?: context
    private val entries = AndroidSystemKeystoreV1()
    private val attemptedAliases = HashSet<String>()

    internal fun requireOriginal() = requireOriginalOwner()

    // These exact private members are retained by the client-android consumer rules for JNI.
    private fun requireOriginalFromNative() = requireOriginalOwner()
    private fun transcriptFromNative(): ByteArray = challenge.transcriptBytes().also { requireOriginalOwner() }
    private fun apiLevelFromNative(): Int = Build.VERSION.SDK_INT.also { requireOriginalOwner() }
    private fun noBackupRootFromNative(): String {
        requireOriginalOwner()
        check(Build.VERSION.SDK_INT >= 26 && !context.isDeviceProtectedStorage) {
            "Authentication intent requires credential-encrypted API26+ storage"
        }
        check(checkNotNull(context.getSystemService(UserManager::class.java)).isUserUnlocked) {
            "Protected authentication intent storage is temporarily unavailable"
        }
        val root = context.noBackupFilesDir.canonicalFile
        check(root.isDirectory && root.isAbsolute)
        return root.path.also { requireOriginalOwner() }
    }

    private fun probeFromNative(slot: ByteArray): NativeFirstDeviceAuthPlatformReplyV1 {
        requireOriginalOwner()
        return try {
            when (val result = FirstDeviceAuthAndroidKeyV1.openExisting(slot.copyOf(), challenge, requireOriginalOwner)) {
                is FirstDeviceAuthAndroidKeyV1.Lookup.Present ->
                    NativeFirstDeviceAuthPlatformReplyV1(0, result.key.publicKeySec1Bytes())
                else -> NativeFirstDeviceAuthPlatformReplyV1(2, byteArrayOf())
            }.also { requireOriginalOwner() }
        } catch (failure: Exception) {
            requireOriginalOwner()
            NativeFirstDeviceAuthPlatformReplyV1(2, byteArrayOf())
        }
    }

    /** Reachable only by Native after durable intent and consumed grant; never from restore. */
    private fun generateFromConsumedNativeIntent(slot: ByteArray, digest: ByteArray,
        strongBox: Boolean): NativeFirstDeviceAuthPlatformReplyV1 = synchronized(attemptedAliases) {
        requireOriginalOwner()
        val alias = firstDeviceAuthAliasV1(slot)
        require(digest.contentEquals(challenge.attestationChallengeBytes()))
        check(Build.VERSION.SDK_INT >= 26 && (!strongBox || Build.VERSION.SDK_INT >= 28))
        check(attemptedAliases.add(alias)) { "Authentication alias generation was already consumed" }
        try {
            // Positive lookup is usable on all supported APIs. A raw null on API26–30 is
            // still UNKNOWN: this private Native original grant, not null, authorizes the
            // first generation. A throwing getKey never permits generation or fallback.
            if (entries.getKey(alias) != null) return@synchronized NativeFirstDeviceAuthPlatformReplyV1(2, byteArrayOf())
            requireOriginalOwner()
            val builder = KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_SIGN)
                .setAlgorithmParameterSpec(ECGenParameterSpec("secp256r1"))
                .setDigests(KeyProperties.DIGEST_SHA256)
                .setUserAuthenticationRequired(false)
                .setAttestationChallenge(digest.copyOf())
            val generator = KeyPairGenerator.getInstance(KeyProperties.KEY_ALGORITHM_EC, "AndroidKeyStore")
            if (strongBox) FirstDeviceAuthStrongBoxApi28V1.generate(generator, builder)
            else { generator.initialize(builder.build()); generator.generateKeyPair() }
            requireOriginalOwner()
            // Genuine readback rejects software keys, changed chains, wrong purpose,
            // authentication-bound keys, wrong hardware level and changed challenge.
            val observed = probeFromNative(slot)
            if (observed.status == 0) {
                val chain = checkNotNull(entries.getCertificateChain(alias)).map {
                    it as? X509Certificate ?: error("Authentication chain is not X509")
                }
                val level = Original.persistentAppHardwareSecurityLevel(chain, digest.copyOf())
                require(level == if (strongBox) AttestationResult.SecurityLevel.STRONG_BOX
                    else AttestationResult.SecurityLevel.TRUSTED_ENVIRONMENT) {
                    "Authentication key hardware differs from the consumed native plan"
                }
            }
            requireOriginalOwner()
            observed
        } catch (typed: FirstDeviceAuthStrongBoxUnavailableV1) {
            requireOriginalOwner()
            NativeFirstDeviceAuthPlatformReplyV1(1, byteArrayOf())
        } catch (failure: Exception) {
            requireOriginalOwner()
            NativeFirstDeviceAuthPlatformReplyV1(2, byteArrayOf())
        }
    }
}

private class FirstDeviceAuthStrongBoxUnavailableV1(cause: Throwable) : Exception(cause)
private object FirstDeviceAuthStrongBoxApi28V1 {
    fun generate(generator: KeyPairGenerator, builder: KeyGenParameterSpec.Builder) {
        try {
            generator.initialize(builder.setIsStrongBoxBacked(true).build())
            generator.generateKeyPair()
        } catch (typed: StrongBoxUnavailableException) { throw FirstDeviceAuthStrongBoxUnavailableV1(typed) }
    }
}

/** JNI-only data carrier. Its status supplies no app authentication or Wallet authority. */
internal class NativeFirstDeviceAuthPlatformReplyV1(@JvmField val status: Int, original: ByteArray) {
    private val original = original.copyOf()
    fun bytes(): ByteArray = original.copyOf()
}

/** JNI-only original key projection, kept intact across R8 shrinking. */
internal class NativeFirstDeviceAuthKeyReplyV1(@JvmField val status: Int, slot: ByteArray, publicKey: ByteArray) {
    private val slot = slot.copyOf()
    private val publicKey = publicKey.copyOf()
    fun slotBytes(): ByteArray = slot.copyOf()
    fun publicKeyBytes(): ByteArray = publicKey.copyOf()
}

internal object NativeFirstDeviceAuthKeyJniV1 {
    init { System.loadLibrary("connect_norito_bridge") }
    external fun reserve(platform: FirstDeviceAuthNativePlatformV1, transcript: ByteArray): NativeFirstDeviceAuthKeyReplyV1?
    external fun restore(platform: FirstDeviceAuthNativePlatformV1, transcript: ByteArray): NativeFirstDeviceAuthKeyReplyV1?
}
