// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.wallet

import android.content.Context
import java.io.File
import java.io.IOException

// TODO(G2-bridge): wire this adapter as the JNI upcall object of the Rust
// `KagemushaWalletPlatformV1` implementation in
// `crates/connect_norito_bridge/src/platform_jni/kagemusha_wallet_advance.rs` (upcalls run on
// attached worker threads), supply the intent gate from the Rust provider's durable intent, and
// add the matching keep rule to `consumer-rules.pro`.
// TODO(G2-android): declare `android:manageSpaceActivity` with a custody warning screen so
// Settings > Storage offers "Manage space" instead of silently clearing custody (device test).

/**
 * Android platform adapter of the KAGEMUSHA wallet `Advance` provider (spec §§1.2, 2.2, 2.3,
 * 4.2; G2 design rev 2).
 *
 * It supplies exactly what the Rust `KagemushaWalletPlatformV1` trait of
 * `crates/iroha_core_zk/src/kagemusha_v1_state/wallet_advance_v1/platform.rs` needs and nothing
 * else: the tri-state payment-key probe, generation bound to the issuer challenge digest and the
 * enrollment's hardware profile, `SHA256withECDSA` signing that returns the platform DER, key
 * deletion for custody deletion, attestation-chain export, the anchor policy (Android needs no
 * rollback anchor), storage state, the boot identity, the sleep-inclusive monotonic clock and the
 * custody root path. Every custody file, marker and decision stays in Rust.
 *
 * Every answer is tri-state: an error is `Unavailable` (retry), never absence. Absence of a key
 * comes only from `KeyStore.getKey` returning null.
 *
 * **Backup and transfer.** The backup set must be empty: a full-data restore stream that reaches
 * an app without its own backup agent clears its data, including `no_backup`, and its Keystore
 * namespace. This library's manifest therefore sets `android:allowBackup="false"` and
 * `android:dataExtractionRules` / `android:fullBackupContent` rules that exclude every domain for
 * cloud backup and device transfer. The host application must not override these attributes
 * (no `tools:replace`) and must not declare a backup agent; [create] refuses to construct the
 * adapter otherwise, and [keyGenerate] refuses enrollment.
 */
class KagemushaWalletAndroidPlatformV1 internal constructor(
    private val environment: KagemushaWalletAndroidEnvironmentV1,
    keyStore: KagemushaWalletAndroidKeyStoreV1,
    intentGate: KagemushaWalletAndroidIntentGateV1,
) {
    private val paymentKey: KagemushaWalletAndroidPaymentKeyV1

    init {
        kagemushaWalletAndroidBackupRefusalV1(environment)?.let { throw IllegalStateException(it) }
        check(!environment.isDeviceProtectedStorage()) {
            "KAGEMUSHA wallet custody requires the credential-encrypted application context"
        }
        paymentKey = KagemushaWalletAndroidPaymentKeyV1(keyStore, environment, intentGate)
    }

    /** Probe the payment key of [slot] (32 nonzero bytes). */
    fun keyProbe(slot: ByteArray): KagemushaWalletAndroidKeyProbeV1 = paymentKey.probe(slot)

    /**
     * Generate the payment key of [slot], whose intent the Rust provider has made durable, with
     * [challengeDigest] as the attestation challenge under [profile]. Never replaces an entry.
     */
    fun keyGenerate(
        slot: ByteArray,
        challengeDigest: ByteArray,
        profile: KagemushaWalletAndroidKeyProfileV1,
    ): KagemushaWalletAndroidKeyGenerationV1 = paymentKey.generate(slot, challengeDigest, profile)

    /** Sign the exact digest [preimage] with the payment key of [slot]. */
    fun keySign(slot: ByteArray, preimage: ByteArray): KagemushaWalletAndroidSignatureV1 =
        paymentKey.sign(slot, preimage)

    /** Delete the payment key of [slot]; the Rust provider calls this only at custody deletion. */
    fun keyDelete(slot: ByteArray): KagemushaWalletAndroidRemoveV1 = paymentKey.delete(slot)

    /** Export the attestation chain of the payment key of [slot], leaf first. */
    fun attestationChain(slot: ByteArray): KagemushaWalletAndroidAttestationChainV1 =
        paymentKey.attestationChain(slot)

    /** Rust `KagemushaWalletAnchorPolicyV1` tag: Android keeps no rollback anchor. */
    fun anchorPolicyTag(): Int = ANCHOR_POLICY_NOT_REQUIRED_TAG

    /** Null when credential-encrypted storage is available; otherwise why it is not. */
    fun storageState(): KagemushaWalletAndroidUnavailableV1? = try {
        if (environment.isUserUnlocked()) null else KagemushaWalletAndroidUnavailableV1.BEFORE_FIRST_UNLOCK
    } catch (error: Throwable) {
        KagemushaWalletAndroidUnavailableV1.platform(KagemushaWalletAndroidUnavailableV1.PLATFORM_STORAGE)
    }

    /** The current boot's `/proc/sys/kernel/random/boot_id`. */
    fun bootId(): KagemushaWalletAndroidBootIdV1 {
        val uuid = try {
            kagemushaWalletAndroidBootUuidV1(environment.readBootId())
        } catch (error: Throwable) {
            null
        }
        return if (uuid != null) {
            KagemushaWalletAndroidBootIdV1.Present(uuid)
        } else {
            KagemushaWalletAndroidBootIdV1.Unavailable(KagemushaWalletAndroidUnavailableV1.io(0))
        }
    }

    /** `SystemClock.elapsedRealtime()`: `CLOCK_BOOTTIME` milliseconds, including deep sleep. */
    fun monotonicMillis(): Long = environment.elapsedRealtimeMillis()

    /**
     * The custody root: `<canonical noBackupFilesDir>/kagemusha-wallet-v1` of the
     * credential-encrypted context. It is available only after the user unlocked the device; the
     * Rust provider creates and syncs the directory itself.
     */
    fun custodyRoot(): KagemushaWalletAndroidCustodyRootV1 {
        storageState()?.let { return KagemushaWalletAndroidCustodyRootV1.Unavailable(it) }
        val parent = try {
            environment.noBackupFilesDir().canonicalPath
        } catch (error: IOException) {
            return KagemushaWalletAndroidCustodyRootV1.Unavailable(KagemushaWalletAndroidUnavailableV1.io(0))
        } catch (error: Throwable) {
            return KagemushaWalletAndroidCustodyRootV1.Unavailable(
                KagemushaWalletAndroidUnavailableV1.platform(KagemushaWalletAndroidUnavailableV1.PLATFORM_STORAGE),
            )
        }
        if (!File(parent).isAbsolute) {
            return KagemushaWalletAndroidCustodyRootV1.Unavailable(
                KagemushaWalletAndroidUnavailableV1.platform(KagemushaWalletAndroidUnavailableV1.PLATFORM_STORAGE),
            )
        }
        return KagemushaWalletAndroidCustodyRootV1.Present(File(parent, CUSTODY_ROOT_DIR_NAME).path)
    }

    companion object {
        /** Rust `KagemushaWalletAnchorPolicyV1::NotRequired` tag. */
        const val ANCHOR_POLICY_NOT_REQUIRED_TAG: Int = 0

        /** Rust `KAGEMUSHA_WALLET_ROOT_DIR_NAME_V1`. */
        const val CUSTODY_ROOT_DIR_NAME: String = "kagemusha-wallet-v1"

        /**
         * Create the adapter over the application's credential-encrypted context, AndroidKeyStore
         * and [intentGate].
         *
         * @throws IllegalStateException when the application allows backup, declares a backup
         * agent, or runs in a device-protected storage context.
         */
        @JvmStatic
        fun create(context: Context, intentGate: KagemushaWalletAndroidIntentGateV1): KagemushaWalletAndroidPlatformV1 =
            KagemushaWalletAndroidPlatformV1(
                KagemushaWalletAndroidSystemEnvironmentV1(context),
                KagemushaWalletAndroidSystemKeyStoreV1(),
                intentGate,
            )

        /** Keystore alias of [slot] (32 nonzero bytes): `kgm-w1-` and 64 lowercase hex digits. */
        @JvmStatic
        fun paymentKeyAlias(slot: ByteArray): String = kagemushaWalletAndroidAliasV1(slot.copyOf())
    }
}

/**
 * Validated lowercase boot UUID of one `boot_id` text, or null: the trimmed text must be a
 * 36-character hyphenated UUID, as Rust `kagemusha_wallet_boot_id_from_text_v1` requires.
 */
internal fun kagemushaWalletAndroidBootUuidV1(text: String): String? {
    val uuid = text.trim()
    if (uuid.length != 36) return null
    for ((index, character) in uuid.withIndex()) {
        val valid = if (index == 8 || index == 13 || index == 18 || index == 23) {
            character == '-'
        } else {
            character in '0'..'9' || character in 'a'..'f' || character in 'A'..'F'
        }
        if (!valid) return null
    }
    return uuid.lowercase()
}
