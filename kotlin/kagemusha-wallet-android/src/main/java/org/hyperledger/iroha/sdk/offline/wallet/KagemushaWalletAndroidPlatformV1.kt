// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.wallet

import android.content.Context
import android.util.Log
import java.io.File
import java.io.IOException
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AndroidAttestationE1PolicyV1
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AndroidAttestationE1ResultV1
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AndroidKeyAttestationOriginalV1
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AttestationResult
import org.hyperledger.iroha.sdk.offline.KagemushaWalletEnrollmentRefusalV1

// Rust JNI retains this opaque object and calls nativeCall from attached worker threads.
// Native installation authenticates the complete operation/Λ/Ω catalog before wallet admission.
// Host apps may provide android:manageSpaceActivity as optional storage UX. Wallet-controlled
// custody deletion requires an explicit destructive warning (§1.2); OS clear-data is not recovery.

/**
 * Opaque Android platform handle of the KAGEMUSHA wallet `Advance` provider (spec §§1.2, 2.2,
 * 2.3, 4.2; G2 design rev 2).
 *
 * The only public operation exports the E5 evidence of a native-selected enrollment target,
 * with the adapter's own pre-E5 admission check ([enrollmentEvidence]).
 * Its private methods are the JNI upcalls of the Rust
 * `KagemushaWalletPlatformV1` adapter (JNI ignores Kotlin visibility; `consumer-rules.pro` keeps
 * them): the tri-state payment-key probe, generation bound to the issuer challenge digest under
 * the one StrongBox-then-TEE plan, `SHA256withECDSA` signing of the exact 32-byte Poseidon signing
 * message (KeyMint hashes it with SHA-256; never a no-digest mode) that returns the platform DER, key
 * deletion for custody deletion, attestation-chain export, the anchor policy (Android keeps no
 * rollback anchor), storage state and the custody root path. Only the Rust role-checked signers
 * reach the signing upcall, so app code holding the handle cannot sign arbitrary bytes with the
 * payment key through this module. Every custody file, marker and decision stays in Rust.
 *
 * [create] refuses (`IllegalStateException`) unless all of these hold:
 * - API level 26 or higher; API 26–30 initial generation requires Native's one-shot fresh grant;
 * - `android:allowBackup="false"` and no `android:backupAgent` in the merged host manifest;
 * - both backup-rule resources of this library, as resolved in the host app, exclude every
 *   domain for cloud backup, device transfer and legacy full backup (a host resource of the same
 *   name that adds files is refused);
 * - the context is the credential-encrypted application context.
 *
 * A full-data restore stream that reaches an app without its own backup agent clears all of its
 * data, including `no_backup`, and its Keystore namespace, so the backup and transfer set must
 * be empty. A host manifest that replaces `android:dataExtractionRules` or
 * `android:fullBackupContent` with its own resource cannot be detected through public APIs; host
 * apps must not do that (kotlin/README.md).
 */
class KagemushaWalletAndroidPlatformV1 private constructor(
    private val adapter: KagemushaWalletAndroidPlatformAdapterV1,
) {
    private val storageObservations = WalletStorageObservationV1(::publishWalletStorageObservationV1)
    /**
     * E5 evidence of the exact native-selected payment key, right after its generation: the
     * original leaf-first KeyMint chain and this adapter's own check of it (design §2.4). The
     * check applies the Phone and App steps of the enrollment policy's Android admission to the
     * chain, under the Android admission inputs of the selected app and enrollment policies
     * ([AndroidAttestationE1PolicyV1.fromPolicyOriginals]), and reports the first failure as
     * its closed refusal reason. On [KagemushaWalletAndroidEnrollmentEvidenceV1.Ineligible]
     * the app abandons the attempt ([KagemushaWalletEnrollmentV1.abandon]) and words the
     * refusal by its holder class; it never submits that chain. The issuer stays authoritative:
     * [KagemushaWalletAndroidEnrollmentEvidenceV1.Eligible] admits nothing.
     *
     * @throws KagemushaWalletExceptionV1 -5 when storage or the key is unavailable now (retry),
     * -7 when the key differs from the target and -13 when it is definitively absent.
     */
    fun enrollmentEvidence(
        target: KagemushaWalletEnrollmentTargetV1,
        policy: AndroidAttestationE1PolicyV1,
    ): KagemushaWalletAndroidEnrollmentEvidenceV1 {
        val certificates = adapter.enrollmentCertificates(target)
        return when (val checked = AndroidKeyAttestationOriginalV1.e1Admission(certificates, target.challengeDigest(), target.paymentKey(), policy)) {
            is AndroidAttestationE1ResultV1.Eligible -> KagemushaWalletAndroidEnrollmentEvidenceV1.Eligible(
                certificates, checked.securityLevel == AttestationResult.SecurityLevel.STRONG_BOX,
            )
            is AndroidAttestationE1ResultV1.Ineligible -> KagemushaWalletAndroidEnrollmentEvidenceV1.Ineligible(checked.refusal, checked.detail)
        }
    }

    // JNI upcalls. Names and signatures are the bridge contract and are pinned by tests.

    @Suppress("unused")
    private fun keyProbe(slot: ByteArray): KagemushaWalletAndroidKeyProbeV1 = adapter.keyProbe(slot)

    @Suppress("unused")
    private fun keyGenerate(slot: ByteArray, challengeDigest: ByteArray, profileTag: Int): KagemushaWalletAndroidKeyGenerationV1 =
        adapter.keyGenerate(slot, challengeDigest, profileTag)

    @Suppress("unused")
    private fun keySign(slot: ByteArray, message: ByteArray): KagemushaWalletAndroidSignatureV1 =
        adapter.keySign(slot, message)

    @Suppress("unused")
    private fun keyDelete(slot: ByteArray): KagemushaWalletAndroidRemoveV1 = adapter.keyDelete(slot)

    @Suppress("unused")
    private fun attestationChain(slot: ByteArray): KagemushaWalletAndroidAttestationChainV1 = adapter.attestationChain(slot)

    @Suppress("unused")
    private fun anchorPolicyTag(): Int = adapter.anchorPolicyTag()

    @Suppress("unused")
    private fun storageState(): KagemushaWalletAndroidUnavailableV1? = adapter.storageState()

    @Suppress("unused")
    private fun custodyRoot(): KagemushaWalletAndroidCustodyRootV1 = adapter.custodyRoot()

    // One typed JNI handoff keeps result decoding independent of Kotlin sealed-class names.
    // It is private: Rust owns signing (2) and consumes its fresh-enrollment grant before (11).
    @Suppress("unused")
    private fun nativeCall(operation: Int, slot: ByteArray, input: ByteArray, auxiliary: Int): KagemushaWalletNativeReplyV1 =
        when (operation) {
            0 -> when (val result = keyProbe(slot)) {
                is KagemushaWalletAndroidKeyProbeV1.Present -> KagemushaWalletNativeReplyV1(0, bytes = result.publicKeySec1())
                KagemushaWalletAndroidKeyProbeV1.Absent -> KagemushaWalletNativeReplyV1(1)
                is KagemushaWalletAndroidKeyProbeV1.Unavailable -> KagemushaWalletNativeReplyV1.unavailable(result.reason)
            }
            1 -> when (val result = keyGenerate(slot, input, auxiliary)) {
                is KagemushaWalletAndroidKeyGenerationV1.Generated -> KagemushaWalletNativeReplyV1(0, bytes = result.publicKeySec1())
                KagemushaWalletAndroidKeyGenerationV1.AlreadyPresent -> KagemushaWalletNativeReplyV1(3)
                is KagemushaWalletAndroidKeyGenerationV1.Unavailable -> KagemushaWalletNativeReplyV1.unavailable(result.reason)
            }
            2 -> when (val result = keySign(slot, input)) {
                is KagemushaWalletAndroidSignatureV1.Der -> KagemushaWalletNativeReplyV1(0, bytes = result.der())
                is KagemushaWalletAndroidSignatureV1.Unavailable -> KagemushaWalletNativeReplyV1.unavailable(result.reason)
            }
            3 -> when (val result = keyDelete(slot)) {
                KagemushaWalletAndroidRemoveV1.Removed -> KagemushaWalletNativeReplyV1(0)
                is KagemushaWalletAndroidRemoveV1.NotRemoved -> KagemushaWalletNativeReplyV1.unavailable(result.reason)
                is KagemushaWalletAndroidRemoveV1.Uncertain -> KagemushaWalletNativeReplyV1.unavailable(result.reason, 4)
            }
            7 -> storageState()?.let { KagemushaWalletNativeReplyV1.unavailable(it) } ?: KagemushaWalletNativeReplyV1(0)
            9 -> when (val result = custodyRoot()) {
                is KagemushaWalletAndroidCustodyRootV1.Present -> KagemushaWalletNativeReplyV1(0, bytes = result.path.toByteArray(Charsets.UTF_8))
                is KagemushaWalletAndroidCustodyRootV1.Unavailable -> KagemushaWalletNativeReplyV1.unavailable(result.reason)
            }
            10 -> storageState()?.let { KagemushaWalletNativeReplyV1.unavailable(it) }
                ?: KagemushaWalletNativeReplyV1(0, reason = 0, code = adapter.keyGenerationMode())
            11 -> when (val result = adapter.keyGenerateFreshFromNative(slot, input, auxiliary)) {
                is KagemushaWalletAndroidKeyGenerationV1.Generated -> KagemushaWalletNativeReplyV1(0, bytes = result.publicKeySec1())
                KagemushaWalletAndroidKeyGenerationV1.AlreadyPresent -> KagemushaWalletNativeReplyV1(3)
                is KagemushaWalletAndroidKeyGenerationV1.Unavailable -> KagemushaWalletNativeReplyV1.unavailable(result.reason)
            }
            12 -> when(val result=attestationChain(slot)) {
                is KagemushaWalletAndroidAttestationChainV1.Present -> KagemushaWalletNativeReplyV1(0,chain=result.certificatesDer().toTypedArray())
                KagemushaWalletAndroidAttestationChainV1.Absent -> KagemushaWalletNativeReplyV1(1)
                is KagemushaWalletAndroidAttestationChainV1.Unavailable -> KagemushaWalletNativeReplyV1.unavailable(result.reason)
            }
            13 -> when (val result = adapter.keyGenerationRecovery(slot, input, auxiliary)) {
                null -> KagemushaWalletNativeReplyV1(3) // No actual return held; never an absence verdict.
                is KagemushaWalletAndroidKeyGenerationV1.Generated -> KagemushaWalletNativeReplyV1(0, bytes = result.publicKeySec1())
                is KagemushaWalletAndroidKeyGenerationV1.Unavailable -> KagemushaWalletNativeReplyV1.unavailable(result.reason)
                KagemushaWalletAndroidKeyGenerationV1.AlreadyPresent -> KagemushaWalletNativeReplyV1(2)
            }
            else -> KagemushaWalletNativeReplyV1(2)
        }.also { reply ->
            storageObservations.reply(operation, reply)
        }

    companion object {
        /**
         * Create the handle over the application's credential-encrypted context and
         * AndroidKeyStore.
         *
         * @throws IllegalStateException when the device is below API 26, the application allows
         * backup, declares a backup agent or carries backup rules that are not exclude-only, or
         * [context] is a device-protected storage context.
         */
        @JvmStatic
        fun create(context: Context): KagemushaWalletAndroidPlatformV1 = create(
            KagemushaWalletAndroidSystemEnvironmentV1(context),
            KagemushaWalletAndroidSystemKeyStoreV1(),
        )

        internal fun create(
            environment: KagemushaWalletAndroidEnvironmentV1,
            keyStore: KagemushaWalletAndroidKeyStoreV1,
        ): KagemushaWalletAndroidPlatformV1 =
            KagemushaWalletAndroidPlatformV1(KagemushaWalletAndroidPlatformAdapterV1(environment, keyStore))
    }
}

private fun enrollmentCertificatesUnavailable(reason: KagemushaWalletAndroidUnavailableV1): Nothing =
    throw KagemushaWalletExceptionV1(-5, reason.kind.tag, reason.code)

/**
 * E5 evidence of one freshly generated payment key with the Android adapter's own pre-E5 check
 * (design §2.4). Neither form is an admission or a device verdict beyond its closed reason.
 */
sealed class KagemushaWalletAndroidEnrollmentEvidenceV1 {
    /** Every client-checkable admission step passed; submit exactly these certificates at E5. */
    class Eligible internal constructor(certificates: List<ByteArray>, @JvmField val strongBox: Boolean) :
        KagemushaWalletAndroidEnrollmentEvidenceV1() {
        private val chain = certificates.map { it.copyOf() }

        /** Copies of the original leaf-first DER certificates for `prepareRequest`. */
        fun certificatesDer(): List<ByteArray> = chain.map { it.copyOf() }

        override fun toString(): String = "Eligible(${chain.size} certificates, strongBox=$strongBox)"
    }

    /**
     * The first failing check as its closed §2.4 reason. Abandon the attempt; the holder class
     * says whether to start again (App, Attempt) or not on this phone (Phone).
     */
    class Ineligible internal constructor(
        @JvmField val refusal: KagemushaWalletEnrollmentRefusalV1,
        @JvmField val detail: String,
    ) : KagemushaWalletAndroidEnrollmentEvidenceV1() {
        override fun toString(): String = "Ineligible(${refusal.name})"
    }
}

/** Implementation of the platform upcalls; see [KagemushaWalletAndroidPlatformV1]. */
internal class KagemushaWalletAndroidPlatformAdapterV1(
    private val environment: KagemushaWalletAndroidEnvironmentV1,
    keyStore: KagemushaWalletAndroidKeyStoreV1,
) {
    private val paymentKey: KagemushaWalletAndroidPaymentKeyV1

    init {
        check(environment.apiLevel >= KAGEMUSHA_WALLET_ANDROID_MIN_API_V1) {
            "KAGEMUSHA wallet custody requires API 26 or later"
        }
        kagemushaWalletAndroidCustodyRefusalV1(environment)?.let { throw IllegalStateException(it) }
        check(!environment.isDeviceProtectedStorage()) {
            "KAGEMUSHA wallet custody requires the credential-encrypted application context"
        }
        paymentKey = KagemushaWalletAndroidPaymentKeyV1(keyStore, environment)
    }

    /** Probe the payment key of [slot] (32 nonzero bytes). */
    fun keyProbe(slot: ByteArray): KagemushaWalletAndroidKeyProbeV1 = paymentKey.probe(slot)

    /**
     * Generate the payment key of [slot] with [challengeDigest] as the attestation challenge.
     * [profileTag] must be the Android platform's only Rust `KagemushaWalletKeyProfileV1` tag
     * (SecureElementOrTee, 2); any other tag fails closed. Never replaces an entry.
     */
    fun keyGenerate(slot: ByteArray, challengeDigest: ByteArray, profileTag: Int): KagemushaWalletAndroidKeyGenerationV1 {
        kagemushaWalletAndroidRequireKeyProfileV1(profileTag)
        return paymentKey.generate(slot, challengeDigest)
    }

    /** Recover only a positive generate return held by this same platform owner; never generates. */
    fun keyGenerationRecovery(slot: ByteArray, challengeDigest: ByteArray, profileTag: Int): KagemushaWalletAndroidKeyGenerationV1? {
        kagemushaWalletAndroidRequireKeyProfileV1(profileTag)
        return paymentKey.recoverGeneration(slot, challengeDigest)
    }

    /** Actual JNI generation mode: 1 needs Native fresh provenance; 0 has definitive absence. */
    fun keyGenerationMode(): Int = if (environment.apiLevel < KAGEMUSHA_WALLET_ANDROID_KEYSTORE2_API_V1) 1 else 0

    /** Only nativeCall operation 11 reaches this after consuming Native's bound fresh grant. */
    fun keyGenerateFreshFromNative(slot: ByteArray, challengeDigest: ByteArray, profileTag: Int): KagemushaWalletAndroidKeyGenerationV1 {
        kagemushaWalletAndroidRequireKeyProfileV1(profileTag)
        storageState()?.let { return KagemushaWalletAndroidKeyGenerationV1.Unavailable(it) }
        return paymentKey.generateFreshFromNative(slot, challengeDigest)
    }

    /**
     * Sign the exact 32-byte signing [message] (the Rust `KagemushaWalletSignMessageV1` bytes)
     * with the payment key of [slot] through `SHA256withECDSA`.
     */
    fun keySign(slot: ByteArray, message: ByteArray): KagemushaWalletAndroidSignatureV1 = paymentKey.sign(slot, message)

    /** Delete the payment key of [slot]; the Rust provider calls this only at custody deletion. */
    fun keyDelete(slot: ByteArray): KagemushaWalletAndroidRemoveV1 = paymentKey.delete(slot)

    /** Export the attestation chain of the payment key of [slot], leaf first. */
    fun attestationChain(slot: ByteArray): KagemushaWalletAndroidAttestationChainV1 = paymentKey.attestationChain(slot)

    fun enrollmentAttestationChain(slot: ByteArray, expectedPaymentKey: ByteArray): KagemushaWalletAndroidAttestationChainV1 =
        paymentKey.enrollmentAttestationChain(slot, expectedPaymentKey)

    /** Original DER for the exact native-selected payment key, with no attestation verdict. */
    fun enrollmentCertificates(target: KagemushaWalletEnrollmentTargetV1): List<ByteArray> {
        storageState()?.let { enrollmentCertificatesUnavailable(it) }
        when (val key = keyProbe(target.slot())) {
            is KagemushaWalletAndroidKeyProbeV1.Present ->
                if (!key.publicKeySec1().contentEquals(target.paymentKey())) throw KagemushaWalletExceptionV1(-7)
            KagemushaWalletAndroidKeyProbeV1.Absent -> throw KagemushaWalletExceptionV1(-13)
            is KagemushaWalletAndroidKeyProbeV1.Unavailable -> enrollmentCertificatesUnavailable(key.reason)
        }
        val result = when (val chain = attestationChain(target.slot())) {
            is KagemushaWalletAndroidAttestationChainV1.Present -> chain.certificatesDer()
            KagemushaWalletAndroidAttestationChainV1.Absent -> throw KagemushaWalletExceptionV1(-13)
            is KagemushaWalletAndroidAttestationChainV1.Unavailable -> enrollmentCertificatesUnavailable(chain.reason)
        }
        storageState()?.let { enrollmentCertificatesUnavailable(it) }
        return result
    }

    /** Rust `KagemushaWalletAnchorPolicyV1` tag: Android keeps no rollback anchor. */
    fun anchorPolicyTag(): Int = KAGEMUSHA_WALLET_ANDROID_ANCHOR_NOT_REQUIRED_TAG_V1

    /** Null when credential-encrypted storage is available; otherwise why it is not. */
    fun storageState(): KagemushaWalletAndroidUnavailableV1? = try {
        if (environment.isUserUnlocked()) null else KagemushaWalletAndroidUnavailableV1.BEFORE_FIRST_UNLOCK
    } catch (error: Throwable) {
        KagemushaWalletAndroidUnavailableV1.platform(KagemushaWalletAndroidUnavailableV1.PLATFORM_STORAGE)
    }

    /**
     * The custody root: `<canonical noBackupFilesDir>/kagemusha-wallet-v1` of the
     * credential-encrypted context. It is available only after the user unlocked the device; the
     * SDK prepares the fixed directory and its durable first-initialization witness before
     * Native opens the existing root. Missing witnessed custody is never recreated.
     */
    fun custodyRoot(): KagemushaWalletAndroidCustodyRootV1 {
        storageState()?.let { return KagemushaWalletAndroidCustodyRootV1.Unavailable(it) }
        val root = try {
            environment.preparedCustodyRoot().path
        } catch (error: IOException) {
            val reason = KagemushaWalletAndroidUnavailableV1.io(0)
            observeWalletStoragePreparationFailureV1(WalletStoragePreparationFailureV1.IO, reason,
                publish = ::publishWalletStorageObservationV1)
            return KagemushaWalletAndroidCustodyRootV1.Unavailable(reason)
        } catch (error: Throwable) {
            val reason = KagemushaWalletAndroidUnavailableV1.platform(KagemushaWalletAndroidUnavailableV1.PLATFORM_STORAGE)
            val diagnostic = error as? WalletStoragePreparationExceptionV1
            observeWalletStoragePreparationFailureV1(diagnostic?.failure ?: WalletStoragePreparationFailureV1.PLATFORM,
                reason, diagnostic?.witnessMismatch ?: 0, ::publishWalletStorageObservationV1)
            return KagemushaWalletAndroidCustodyRootV1.Unavailable(reason)
        }
        if (!File(root).isAbsolute) {
            val reason = KagemushaWalletAndroidUnavailableV1.platform(KagemushaWalletAndroidUnavailableV1.PLATFORM_STORAGE)
            observeWalletStoragePreparationFailureV1(WalletStoragePreparationFailureV1.PATH, reason,
                publish = ::publishWalletStorageObservationV1)
            return KagemushaWalletAndroidCustodyRootV1.Unavailable(reason)
        }
        return KagemushaWalletAndroidCustodyRootV1.Present(root)
    }
}

/** Public storage observations only: no path, slot, payload, exception text or key material.
 * Diagnostics must never change a platform answer, including when the log sink fails. */
internal class WalletStorageObservationV1(private val publish: (String) -> Unit) {
    private var observedSuccesses = 0

    fun reply(operation: Int, reply: KagemushaWalletNativeReplyV1): KagemushaWalletNativeReplyV1 {
        val name: String
        val bit: Int
        when (operation) {
            7 -> { name = "storage_state"; bit = 1 }
            9 -> { name = "custody_root"; bit = 2 }
            else -> return reply
        }
        // Storage is probed during ordinary wallet work too. Bound success records per
        // platform lifetime; every unavailable answer remains visible without a held lock.
        val report = synchronized(this) {
            if (reply.tag != 0) true else {
                val first = (observedSuccesses and bit) == 0
                observedSuccesses = observedSuccesses or bit
                first
            }
        }
        if (report) runCatching {
            publish("operation=$name status=${reply.tag} reason=${reply.reason} platform_code=${reply.code}")
        }
        return reply
    }
}

internal enum class WalletStoragePreparationFailureV1 {
    IO, PLATFORM, PATH, WITNESS_BINDING, WITNESS_SHAPE, ROOT_MISSING, PRIVACY, IDENTITY,
}

/** The original refusal remains an IllegalStateException with the same fixed message.
 * Only its closed observation category and bounded comparison mask leave this layer. */
internal class WalletStoragePreparationExceptionV1(
    val failure: WalletStoragePreparationFailureV1,
    val witnessMismatch: Int = 0,
    message: String,
) : IllegalStateException(message)

/** Compare only the two already-read local identity frames. Bits identify differing fields,
 * never their values: device=1, inode=2, uid=4, gid=8, malformed shape/prefix=16.
 * This observation cannot admit, rewrite, migrate or rebind a witness. */
internal fun walletStorageWitnessMismatchV1(actual: ByteArray, expected: ByteArray): Int {
    if (actual.size != 32 || expected.size != 32) return 16
    for (index in 0 until 8) if (actual[index] != expected[index]) return 16
    var mask = 0
    for (index in 8 until 32) {
        if (actual[index] != expected[index]) mask = mask or when {
            index < 16 -> 1
            index < 24 -> 2
            index < 28 -> 4
            else -> 8
        }
    }
    return mask
}

internal fun observeWalletStoragePreparationFailureV1(failure: WalletStoragePreparationFailureV1,
    reason: KagemushaWalletAndroidUnavailableV1, witnessMismatch: Int = 0, publish: (String) -> Unit) {
    runCatching {
        val mismatch = if (failure == WalletStoragePreparationFailureV1.WITNESS_BINDING)
            " witness_mismatch=${witnessMismatch and 31}" else ""
        publish("operation=custody_root_prepare status=2 reason=${reason.kind.tag} platform_code=${reason.code} failure=${failure.name}$mismatch")
    }
}

private fun publishWalletStorageObservationV1(message: String) {
    Log.i("IrohaWalletStorage", message)
}

/** Rust `KagemushaWalletAnchorPolicyV1::NotRequired` tag. */
internal const val KAGEMUSHA_WALLET_ANDROID_ANCHOR_NOT_REQUIRED_TAG_V1: Int = 0

/** Rust `KAGEMUSHA_WALLET_ROOT_DIR_NAME_V1`. */
internal const val KAGEMUSHA_WALLET_ANDROID_CUSTODY_DIR_NAME_V1: String = "kagemusha-wallet-v1"
