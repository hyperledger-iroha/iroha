// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.wallet

/**
 * Reason an Android platform answer is not definitive. It is never absence.
 *
 * The kinds mirror the Rust `KagemushaWalletUnavailableV1` of
 * `crates/iroha_core_zk/src/kagemusha_v1_state/wallet_advance_v1/platform.rs` one to one, so the
 * bridge maps [kind] and [code] without interpretation. [code] is the OS error for [Kind.IO] and
 * one of the `PLATFORM_*` constants for [Kind.PLATFORM]; it is 0 for every other kind.
 */
class KagemushaWalletAndroidUnavailableV1 private constructor(
    @JvmField val kind: Kind,
    @JvmField val code: Int,
) {
    /** Kind of an unavailable answer; [tag] is the stable bridge value. */
    enum class Kind(@JvmField val tag: Int) {
        /** Protected storage or the key store is locked. */
        LOCKED(0),

        /** The user has not unlocked the device since boot (credential-encrypted storage). */
        BEFORE_FIRST_UNLOCK(1),

        /** Another opener holds the custody lock, or a concurrent change was observed. */
        BUSY(2),

        /** An I/O error. */
        IO(3),

        /** A platform API error; [code] names it. */
        PLATFORM(4),

        /** The key exists but refused to sign, or does not match what was generated. */
        KEY_UNUSABLE(5),

        /** The platform reports the key as permanently invalidated; it is never regenerated. */
        PERMANENTLY_INVALIDATED(6),
    }

    override fun equals(other: Any?): Boolean =
        other is KagemushaWalletAndroidUnavailableV1 && other.kind == kind && other.code == code

    override fun hashCode(): Int = kind.hashCode() * 31 + code

    override fun toString(): String = "KagemushaWalletAndroidUnavailableV1($kind, $code)"

    companion object {
        /** Keystore access threw; the entry state is unknown. */
        const val PLATFORM_KEYSTORE: Int = 1

        /** A present key's certificate or attestation chain could not be read. */
        const val PLATFORM_CERTIFICATE: Int = 2

        /** The key to sign with is definitively absent; reconcile classifies the loss. */
        const val PLATFORM_KEY_ABSENT: Int = 3

        /** Key generation failed with an unknown outcome; probe again before acting. */
        const val PLATFORM_GENERATION_FAILED: Int = 4

        /** The enrollment profile requires StrongBox and the device has none. */
        const val PLATFORM_STRONGBOX_UNAVAILABLE: Int = 5

        /** The slot's enrollment intent is not durable, so its alias is not yet recorded. */
        const val PLATFORM_INTENT_NOT_DURABLE: Int = 6

        /** The slot's enrollment intent could not be read. */
        const val PLATFORM_INTENT_UNAVAILABLE: Int = 7

        /** The application allows backup or has its own backup agent; enrollment is refused. */
        const val PLATFORM_BACKUP_ENABLED: Int = 8

        /** Signing failed for a reason other than the key itself. */
        const val PLATFORM_SIGN_FAILED: Int = 9

        /** `UserManager` or the no-backup directory could not be queried. */
        const val PLATFORM_STORAGE: Int = 10

        @JvmField val LOCKED = KagemushaWalletAndroidUnavailableV1(Kind.LOCKED, 0)
        @JvmField val BEFORE_FIRST_UNLOCK = KagemushaWalletAndroidUnavailableV1(Kind.BEFORE_FIRST_UNLOCK, 0)
        @JvmField val BUSY = KagemushaWalletAndroidUnavailableV1(Kind.BUSY, 0)
        @JvmField val KEY_UNUSABLE = KagemushaWalletAndroidUnavailableV1(Kind.KEY_UNUSABLE, 0)
        @JvmField val PERMANENTLY_INVALIDATED = KagemushaWalletAndroidUnavailableV1(Kind.PERMANENTLY_INVALIDATED, 0)

        /** An I/O error with its OS error code, or 0 when none is available. */
        @JvmStatic fun io(code: Int): KagemushaWalletAndroidUnavailableV1 =
            KagemushaWalletAndroidUnavailableV1(Kind.IO, code)

        /** A platform error named by one of the `PLATFORM_*` constants. */
        @JvmStatic fun platform(code: Int): KagemushaWalletAndroidUnavailableV1 =
            KagemushaWalletAndroidUnavailableV1(Kind.PLATFORM, code)
    }
}

/** Hardware key policy of one enrollment; [tag] equals Rust `KagemushaWalletKeyProfileV1::tag`. */
enum class KagemushaWalletAndroidKeyProfileV1(@JvmField val tag: Int) {
    /** StrongBox only: a device without StrongBox refuses enrollment. */
    SECURE_ELEMENT(1),

    /** StrongBox when present, otherwise the TEE, but only while the slot's key is still absent. */
    SECURE_ELEMENT_OR_TEE(2),
    ;

    companion object {
        /** Profile of a stored tag, or null for an unknown tag. */
        @JvmStatic fun fromTag(tag: Int): KagemushaWalletAndroidKeyProfileV1? = entries.firstOrNull { it.tag == tag }
    }
}

/** Hardware that holds a generated payment key. */
enum class KagemushaWalletAndroidSecurityLevelV1 {
    /** Android StrongBox. */
    STRONGBOX,

    /** The trusted execution environment. */
    TRUSTED_ENVIRONMENT,
}

/** Tri-state probe of one slot's payment key. */
sealed class KagemushaWalletAndroidKeyProbeV1 {
    /** The key exists; its public key is the canonical 65-byte uncompressed P-256 point. */
    class Present internal constructor(publicKeySec1: ByteArray) : KagemushaWalletAndroidKeyProbeV1() {
        private val point = publicKeySec1.copyOf()

        /** The canonical uncompressed SEC1 public key. */
        fun publicKeySec1(): ByteArray = point.copyOf()

        override fun toString(): String = "Present"
    }

    /** `KeyStore.getKey` definitively reported no key under the slot's alias. */
    object Absent : KagemushaWalletAndroidKeyProbeV1() {
        override fun toString(): String = "Absent"
    }

    /** No definitive answer; retry. Never absence. */
    class Unavailable internal constructor(
        @JvmField val reason: KagemushaWalletAndroidUnavailableV1,
    ) : KagemushaWalletAndroidKeyProbeV1() {
        override fun toString(): String = "Unavailable($reason)"
    }
}

/** Outcome of one payment-key generation request. */
sealed class KagemushaWalletAndroidKeyGenerationV1 {
    /** A new key was generated under the slot's alias and checked against the request. */
    class Generated internal constructor(
        publicKeySec1: ByteArray,
        @JvmField val securityLevel: KagemushaWalletAndroidSecurityLevelV1,
    ) : KagemushaWalletAndroidKeyGenerationV1() {
        private val point = publicKeySec1.copyOf()

        /** The canonical uncompressed SEC1 public key. */
        fun publicKeySec1(): ByteArray = point.copyOf()

        override fun toString(): String = "Generated($securityLevel)"
    }

    /** A key exists or existed under the alias; nothing was generated and the slot is used up. */
    object AlreadyPresent : KagemushaWalletAndroidKeyGenerationV1() {
        override fun toString(): String = "AlreadyPresent"
    }

    /** No key was generated, or the outcome is unknown; probe again before acting. */
    class Unavailable internal constructor(
        @JvmField val reason: KagemushaWalletAndroidUnavailableV1,
    ) : KagemushaWalletAndroidKeyGenerationV1() {
        override fun toString(): String = "Unavailable($reason)"
    }
}

/** Outcome of one payment-key signature. */
sealed class KagemushaWalletAndroidSignatureV1 {
    /** The platform's strict DER `SHA256withECDSA` signature, unmodified; Rust freezes it. */
    class Der internal constructor(der: ByteArray) : KagemushaWalletAndroidSignatureV1() {
        private val bytes = der.copyOf()

        /** The DER signature. */
        fun der(): ByteArray = bytes.copyOf()

        override fun toString(): String = "Der(${bytes.size} bytes)"
    }

    /** The key did not sign; the operation stays pending. */
    class Unavailable internal constructor(
        @JvmField val reason: KagemushaWalletAndroidUnavailableV1,
    ) : KagemushaWalletAndroidSignatureV1() {
        override fun toString(): String = "Unavailable($reason)"
    }
}

/** Outcome of one payment-key deletion (custody deletion only). */
sealed class KagemushaWalletAndroidRemoveV1 {
    /** The key is definitively absent. */
    object Removed : KagemushaWalletAndroidRemoveV1() {
        override fun toString(): String = "Removed"
    }

    /** The key is still present. */
    class NotRemoved internal constructor(
        @JvmField val reason: KagemushaWalletAndroidUnavailableV1,
    ) : KagemushaWalletAndroidRemoveV1() {
        override fun toString(): String = "NotRemoved($reason)"
    }

    /** Whether the key is gone is unknown. */
    class Uncertain internal constructor(
        @JvmField val reason: KagemushaWalletAndroidUnavailableV1,
    ) : KagemushaWalletAndroidRemoveV1() {
        override fun toString(): String = "Uncertain($reason)"
    }
}

/** Tri-state export of one payment key's attestation chain, leaf first. */
sealed class KagemushaWalletAndroidAttestationChainV1 {
    /** The DER certificates of the attestation chain, leaf first. */
    class Present internal constructor(chainDer: List<ByteArray>) : KagemushaWalletAndroidAttestationChainV1() {
        private val chain = chainDer.map { it.copyOf() }

        /** Copies of the DER certificates, leaf first. */
        fun certificatesDer(): List<ByteArray> = chain.map { it.copyOf() }

        override fun toString(): String = "Present(${chain.size} certificates)"
    }

    /** The key is definitively absent. */
    object Absent : KagemushaWalletAndroidAttestationChainV1() {
        override fun toString(): String = "Absent"
    }

    /** No definitive answer; retry. Never absence. */
    class Unavailable internal constructor(
        @JvmField val reason: KagemushaWalletAndroidUnavailableV1,
    ) : KagemushaWalletAndroidAttestationChainV1() {
        override fun toString(): String = "Unavailable($reason)"
    }
}

/** Boot identity text: the lowercase hyphenated UUID of `/proc/sys/kernel/random/boot_id`. */
sealed class KagemushaWalletAndroidBootIdV1 {
    /** The validated boot UUID; the bridge hashes it with `kagemusha_wallet_boot_id_from_text_v1`. */
    class Present internal constructor(@JvmField val uuid: String) : KagemushaWalletAndroidBootIdV1() {
        override fun toString(): String = "Present"
    }

    /** The boot identity cannot be read; every file is then treated as written in this boot. */
    class Unavailable internal constructor(
        @JvmField val reason: KagemushaWalletAndroidUnavailableV1,
    ) : KagemushaWalletAndroidBootIdV1() {
        override fun toString(): String = "Unavailable($reason)"
    }
}

/** Custody root path, or why it is not available now. */
sealed class KagemushaWalletAndroidCustodyRootV1 {
    /** Absolute canonical path of the custody root; Rust creates the directory durably. */
    class Present internal constructor(@JvmField val path: String) : KagemushaWalletAndroidCustodyRootV1() {
        override fun toString(): String = "Present($path)"
    }

    /** Credential-encrypted storage is not available; retry. */
    class Unavailable internal constructor(
        @JvmField val reason: KagemushaWalletAndroidUnavailableV1,
    ) : KagemushaWalletAndroidCustodyRootV1() {
        override fun toString(): String = "Unavailable($reason)"
    }
}

/** Durable state of a slot's enrollment intent, as the Rust provider reports it. */
enum class KagemushaWalletAndroidIntentStateV1 {
    /** `intent.norito` naming the slot is durable. */
    DURABLE,

    /** The slot has no durable intent. */
    ABSENT,

    /** The intent could not be read; retry. */
    UNAVAILABLE,
}

/**
 * Hook confirming that the Rust provider has made a slot's enrollment intent durable.
 *
 * The slot is a fresh random identity and the Keystore alias is derived from it
 * (`kgm-w1-<slot hex>`), so a durable intent records the alias before any key exists under it.
 * The adapter calls this immediately before generating and generates only on
 * [KagemushaWalletAndroidIntentStateV1.DURABLE]; a thrown exception counts as unavailable.
 */
fun interface KagemushaWalletAndroidIntentGateV1 {
    /** Durable state of the intent of [slot] (32 bytes; the callee must not retain the array). */
    fun intentState(slot: ByteArray): KagemushaWalletAndroidIntentStateV1
}
