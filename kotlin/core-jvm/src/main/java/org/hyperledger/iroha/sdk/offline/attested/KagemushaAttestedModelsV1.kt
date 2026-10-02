// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.attested

import org.hyperledger.iroha.sdk.offline.attested.KagemushaAttestedNorito.MODEL

/*
 * Canonical models of the `iroha:kagemusha:v1:attested-app` suite.
 *
 * Every signature is ECDSA P-256/SHA-256 over `domain || canonical unsigned frame`, and every
 * identifier or digest is computed over the unsigned frame, so ECDSA malleability can never
 * change an identifier. Decoders are strict: they accept only the canonical encoding and
 * re-encode every value to prove it.
 */

/** Platform recorded in a device certificate. There is deliberately no test value. */
enum class KagemushaAttestedPlatform(val code: Int) {
    ANDROID_STRONGBOX(1),
    ANDROID_TEE(2),
    APPLE_SECURE_ENCLAVE(3),
    ;

    companion object {
        @JvmStatic
        fun fromCode(code: Int): KagemushaAttestedPlatform? = entries.firstOrNull { it.code == code }
    }
}

/** The six KAGEMUSHA V1 relation names, reused as attested transition kinds. */
enum class KagemushaTransitionKind(val code: Int) {
    BOOTSTRAP(0),
    MINT_FOLD(1),
    SEND_SPLIT(2),
    RECEIVE_FOLD(3),
    REDEEM_SPLIT(4),
    ROTATE(5),
    ;

    /** True for kinds that count toward `unsynced_out`. */
    val isOutgoing: Boolean get() = this == SEND_SPLIT || this == REDEEM_SPLIT

    companion object {
        @JvmStatic
        fun fromCode(code: Int): KagemushaTransitionKind? = entries.firstOrNull { it.code == code }
    }
}

/** Issuer revocation reasons. Integrity, Superseded and Closed block paying only. */
enum class KagemushaRevocationReason(val code: Int) {
    FRAUD(0),
    LOST(1),
    INTEGRITY(2),
    SUPERSEDED(3),
    CLOSED(4),
    ;

    /** True when receivers must refuse and the issuer never delivers. */
    val neverDelivered: Boolean get() = this == FRAUD || this == LOST

    companion object {
        @JvmStatic
        fun fromCode(code: Int): KagemushaRevocationReason? = entries.firstOrNull { it.code == code }
    }
}

/** Issuer-side redemption progress reported in a sync receipt. A redemption is never cancelled. */
enum class KagemushaRedemptionState(val code: Int) {
    PENDING(0),
    QUEUED(1),
    PAID(2),
    FROZEN(3),
    ;

    companion object {
        @JvmStatic
        fun fromCode(code: Int): KagemushaRedemptionState? = entries.firstOrNull { it.code == code }
    }
}

internal object KagemushaAttestedLimits {
    const val VERSION: Int = 1
    const val MAXIMUM_STRING_BYTES: Int = 1_024
    const val MAXIMUM_ISSUER_KEYS: Int = 16
    const val MAXIMUM_TIERS: Int = 16
    const val MAXIMUM_DELTA_ENTRIES: Int = 8
    const val MAXIMUM_CRL_ENTRIES: Int = 65_536
    const val MAXIMUM_RECEIPT_DELIVERIES: Int = 1_024
    const val MAXIMUM_RECEIPT_REDEMPTIONS: Int = 1_024
    const val MAXIMUM_POLICY_BYTES: Int = 32 * 1_024

    const val DESCRIPTOR_BYTES: Int = 96 * 1_024
    const val CERT_BYTES: Int = 512
    const val TRANSITION_BYTES: Int = 512
    const val REQUEST_BYTES: Int = 2_048
    const val PAYMENT_BYTES: Int = 2_048
    const val ACK_BYTES: Int = 512
    const val VOUCHER_BYTES: Int = 512
    const val CRL_BYTES: Int = 4 * 1_024 * 1_024
    const val DELTA_BYTES: Int = 1_024
    const val DELIVERY_BYTES: Int = 512
    const val FORK_EVIDENCE_BYTES: Int = 2_048
    const val RECEIPT_BYTES: Int = 8 * 1_024 * 1_024
}

private fun fixed(bytes: ByteArray, width: Int, name: String): ByteArray {
    require(bytes.size == width) { "$name must be exactly $width bytes" }
    return bytes.copyOf()
}

private fun requireAmount(value: Long, name: String) {
    require(value in 0..KagemushaAmount.MAXIMUM_MINOR) { "$name is outside 0..=10^15 minor units" }
}

private fun requireU8(value: Int, name: String) = require(value in 0..0xff) { "$name is not a u8" }

private fun requireNonNegative(value: Long, name: String) = require(value >= 0) { "$name must be non-negative" }

private fun le64(value: Long): ByteArray = ByteArray(8) { index -> (value ushr (8 * index)).toByte() }

private fun lengthPrefixed(text: String): ByteArray {
    val utf8 = text.toByteArray(Charsets.UTF_8)
    return le64(utf8.size.toLong()) + utf8
}

/** Identifier derivations shared by the SDK, the issuer and the vectors. */
object KagemushaAttestedIds {
    /** `scheme_id = H(D_scheme-id || len(chain) chain || len(asset) asset || len(reserve) reserve || root_pk)`. */
    @JvmStatic
    fun schemeId(chainId: String, assetDefinitionId: String, reserveAccountId: String, rootPublicKey: ByteArray): ByteArray =
        KagemushaAttestedDomain.SCHEME_ID.hash(
            lengthPrefixed(chainId),
            lengthPrefixed(assetDefinitionId),
            lengthPrefixed(reserveAccountId),
            fixed(rootPublicKey, 65, "root public key"),
        )

    /** `device_id = H(D_device-id || scheme_id || device_pk)`. */
    @JvmStatic
    fun deviceId(schemeId: ByteArray, devicePublicKey: ByteArray): ByteArray =
        KagemushaAttestedDomain.DEVICE_ID.hash(fixed(schemeId, 32, "scheme_id"), fixed(devicePublicKey, 65, "device_pk"))

    /** `voucher_id = H(D_voucher || tx_hash || load_id)`. */
    @JvmStatic
    fun voucherId(txHash: ByteArray, loadId: ByteArray): ByteArray =
        KagemushaAttestedDomain.VOUCHER.hash(fixed(txHash, 32, "tx_hash"), fixed(loadId, 16, "load_id"))

    /** `account_digest = SHA-256(UTF-8 account id)`. */
    @JvmStatic
    fun accountDigest(accountId: String): ByteArray {
        require(accountId.isNotEmpty() && accountId.length <= KagemushaAttestedLimits.MAXIMUM_STRING_BYTES) {
            "account id is empty or too long"
        }
        return KagemushaAttestedCrypto.sha256(accountId.toByteArray(Charsets.UTF_8))
    }

    /** `redemption_id = H(D_redemption-id || device_id || seq || account_digest || amount)`. */
    @JvmStatic
    fun redemptionId(deviceId: ByteArray, seq: Long, accountDigest: ByteArray, amount: Long): ByteArray =
        KagemushaAttestedDomain.REDEMPTION_ID.hash(
            fixed(deviceId, 32, "device_id"),
            le64(seq),
            fixed(accountDigest, 32, "account_digest"),
            le64(amount),
        )

    /** `binding = H(D_load-binding || scheme_id || device_id || load_id || amount)`. */
    @JvmStatic
    fun loadBinding(schemeId: ByteArray, deviceId: ByteArray, loadId: ByteArray, amount: Long): ByteArray =
        KagemushaAttestedDomain.LOAD_BINDING.hash(
            fixed(schemeId, 32, "scheme_id"),
            fixed(deviceId, 32, "device_id"),
            fixed(loadId, 16, "load_id"),
            le64(amount),
        )

    /** Sync attestation binding `H(D_sync || device_id || sync_nonce || head || seq)`. */
    @JvmStatic
    fun syncBinding(deviceId: ByteArray, syncNonce: ByteArray, head: ByteArray, seq: Long): ByteArray =
        KagemushaAttestedDomain.SYNC.hash(
            fixed(deviceId, 32, "device_id"),
            fixed(syncNonce, 32, "sync_nonce"),
            fixed(head, 32, "head"),
            le64(seq),
        )

    /**
     * Enrollment transcript
     * `E = D_enroll || scheme_id || server_nonce || client_nonce || account_digest || platform ||
     * attested_key_id || signing_pk`. Android uses all-zero `attested_key_id` and `signing_pk`.
     */
    @JvmStatic
    fun enrollmentTranscript(
        schemeId: ByteArray,
        serverNonce: ByteArray,
        clientNonce: ByteArray,
        accountDigest: ByteArray,
        platform: KagemushaAttestedPlatform,
        attestedKeyId: ByteArray,
        signingPublicKey: ByteArray,
    ): ByteArray = KagemushaAttestedDomain.ENROLL.message(
        fixed(schemeId, 32, "scheme_id"),
        fixed(serverNonce, 32, "server_nonce"),
        fixed(clientNonce, 32, "client_nonce"),
        fixed(accountDigest, 32, "account_digest"),
        byteArrayOf(platform.code.toByte()),
        fixed(attestedKeyId, 32, "attested_key_id"),
        fixed(signingPublicKey, 65, "signing_pk"),
    )
}

/** One issuer P-256 key listed by the scheme descriptor. */
class KagemushaAttestedIssuerKeyV1(
    @JvmField val index: Int,
    publicKey: ByteArray,
    @JvmField val notBeforeMs: Long,
    @JvmField val notAfterMs: Long,
) {
    private val key = KagemushaAttestedCrypto.sec1(KagemushaAttestedCrypto.publicKey(publicKey))

    init {
        requireU8(index, "issuer key index")
        requireNonNegative(notBeforeMs, "issuer key not_before_ms")
        require(notAfterMs >= notBeforeMs) { "issuer key validity window is inverted" }
    }

    fun publicKey(): ByteArray = key.copyOf()

    /** True when [atMs] falls inside the key's validity window. */
    fun validAt(atMs: Long): Boolean = atMs in notBeforeMs..notAfterMs

    internal fun write(out: NoritoOut) {
        out.u8(index)
        out.array(key, 65)
        out.u64(notBeforeMs)
        out.u64(notAfterMs)
    }

    override fun equals(other: Any?): Boolean = other is KagemushaAttestedIssuerKeyV1 && index == other.index &&
        key.contentEquals(other.key) && notBeforeMs == other.notBeforeMs && notAfterMs == other.notAfterMs

    override fun hashCode(): Int = 31 * index + key.contentHashCode()

    internal companion object {
        fun read(input: NoritoIn) = KagemushaAttestedIssuerKeyV1(input.u8(), input.array(65), input.u64(), input.u64())
    }
}

/** Per-tier certificate limits (minor units and milliseconds). */
class KagemushaAttestedTierV1(
    @JvmField val tier: Int,
    @JvmField val maxBalance: Long,
    @JvmField val maxPayment: Long,
    @JvmField val maxUnsyncedOut: Long,
    @JvmField val leaseMs: Long,
) {
    init {
        requireU8(tier, "tier")
        requireAmount(maxBalance, "tier max_balance")
        requireAmount(maxPayment, "tier max_payment")
        requireAmount(maxUnsyncedOut, "tier max_unsynced_out")
        require(leaseMs > 0) { "tier lease must be positive" }
    }

    internal fun write(out: NoritoOut) {
        out.u8(tier)
        out.u64(maxBalance)
        out.u64(maxPayment)
        out.u64(maxUnsyncedOut)
        out.u64(leaseMs)
    }

    override fun equals(other: Any?): Boolean = other is KagemushaAttestedTierV1 && tier == other.tier &&
        maxBalance == other.maxBalance && maxPayment == other.maxPayment &&
        maxUnsyncedOut == other.maxUnsyncedOut && leaseMs == other.leaseMs

    override fun hashCode(): Int = 31 * tier + maxBalance.hashCode()

    internal companion object {
        fun read(input: NoritoIn) = KagemushaAttestedTierV1(input.u8(), input.u64(), input.u64(), input.u64(), input.u64())
    }
}

/** Per-account caps enforced by the issuer. */
class KagemushaAttestedAccountLimitsV1(
    @JvmField val devices: Int,
    @JvmField val dailyLoad: Long,
    @JvmField val dailyUnload: Long,
) {
    init {
        requireU8(devices, "account device cap")
        requireAmount(dailyLoad, "daily load cap")
        requireAmount(dailyUnload, "daily unload cap")
    }

    internal fun write(out: NoritoOut) {
        out.u8(devices)
        out.u64(dailyLoad)
        out.u64(dailyUnload)
    }

    override fun equals(other: Any?): Boolean = other is KagemushaAttestedAccountLimitsV1 &&
        devices == other.devices && dailyLoad == other.dailyLoad && dailyUnload == other.dailyUnload

    override fun hashCode(): Int = 31 * devices + dailyLoad.hashCode()

    internal companion object {
        fun read(input: NoritoIn) = KagemushaAttestedAccountLimitsV1(input.u8(), input.u64(), input.u64())
    }
}

/**
 * Root-signed scheme descriptor. Platform admission policies (Android and Apple, including the
 * vendor attestation-root registry) are evaluated by the issuer, so wallets keep their canonical
 * field bodies verbatim instead of interpreting them.
 */
class KagemushaAttestedSchemeDescriptorV1(
    @JvmField val version: Int,
    schemeId: ByteArray,
    @JvmField val descriptorEpoch: Long,
    @JvmField val chainId: String,
    @JvmField val assetDefinitionId: String,
    @JvmField val assetScale: Int,
    @JvmField val reserveAccountId: String,
    @JvmField val issuerUrl: String,
    issuerKeys: List<KagemushaAttestedIssuerKeyV1>,
    androidPolicy: ByteArray,
    applePolicy: ByteArray,
    tiers: List<KagemushaAttestedTierV1>,
    @JvmField val accountLimits: KagemushaAttestedAccountLimitsV1,
    @JvmField val receiverGraceMs: Long,
    @JvmField val headroomQuantum: Long,
    @JvmField val allowTestDevices: Boolean,
    rootSignature: ByteArray,
) {
    private val scheme = fixed(schemeId, 32, "scheme_id")
    private val android = androidPolicy.copyOf()
    private val apple = applePolicy.copyOf()
    private val signature = fixed(rootSignature, 64, "root signature")

    @JvmField val issuerKeys: List<KagemushaAttestedIssuerKeyV1> = issuerKeys.toList()
    @JvmField val tiers: List<KagemushaAttestedTierV1> = tiers.toList()

    init {
        require(version == KagemushaAttestedLimits.VERSION) { "unsupported descriptor version" }
        requireNonNegative(descriptorEpoch, "descriptor epoch")
        listOf(chainId, assetDefinitionId, reserveAccountId, issuerUrl).forEach {
            require(it.isNotEmpty() && it.toByteArray(Charsets.UTF_8).size <= KagemushaAttestedLimits.MAXIMUM_STRING_BYTES) {
                "descriptor strings must be non-empty and bounded"
            }
        }
        require(assetScale in 0..KagemushaAmount.MAXIMUM_SCALE) { "descriptor asset scale is out of range" }
        require(this.issuerKeys.size in 1..KagemushaAttestedLimits.MAXIMUM_ISSUER_KEYS &&
            this.issuerKeys.map { it.index }.toSet().size == this.issuerKeys.size
        ) { "descriptor issuer keys must be non-empty with unique indexes" }
        require(this.tiers.size in 1..KagemushaAttestedLimits.MAXIMUM_TIERS &&
            this.tiers.map { it.tier }.toSet().size == this.tiers.size
        ) { "descriptor tiers must be non-empty and unique" }
        require(android.size <= KagemushaAttestedLimits.MAXIMUM_POLICY_BYTES &&
            apple.size <= KagemushaAttestedLimits.MAXIMUM_POLICY_BYTES
        ) { "descriptor platform policy is too large" }
        requireNonNegative(receiverGraceMs, "receiver grace")
        require(headroomQuantum in 1..KagemushaAmount.MAXIMUM_MINOR) { "headroom quantum must be positive" }
    }

    fun schemeId(): ByteArray = scheme.copyOf()
    fun androidPolicyBody(): ByteArray = android.copyOf()
    fun applePolicyBody(): ByteArray = apple.copyOf()
    fun rootSignature(): ByteArray = signature.copyOf()

    /** The issuer key with [index], if listed. */
    fun issuerKey(index: Int): KagemushaAttestedIssuerKeyV1? = issuerKeys.firstOrNull { it.index == index }

    /** The tier policy with [tier], if listed. */
    fun tier(tier: Int): KagemushaAttestedTierV1? = tiers.firstOrNull { it.tier == tier }

    private fun writeUnsigned(out: NoritoOut) {
        out.u16(version)
        out.array(scheme, 32)
        out.u64(descriptorEpoch)
        out.string(chainId)
        out.string(assetDefinitionId)
        out.u8(assetScale)
        out.string(reserveAccountId)
        out.string(issuerUrl)
        out.vec(issuerKeys) { element, key -> key.write(element) }
        out.opaque(android)
        out.opaque(apple)
        out.vec(tiers) { element, tier -> tier.write(element) }
        out.nested { accountLimits.write(it) }
        out.u64(receiverGraceMs)
        out.u64(headroomQuantum)
        out.bool(allowTestDevices)
    }

    /** Canonical unsigned frame covered by the root signature. */
    fun unsignedBytes(): ByteArray = KagemushaAttestedNorito.frame(SCHEMA, KagemushaAttestedNorito.payload(::writeUnsigned))

    /** `H(D_descriptor || unsigned)`. */
    fun digest(): ByteArray = KagemushaAttestedDomain.DESCRIPTOR.hash(unsignedBytes())

    fun encode(): ByteArray = KagemushaAttestedNorito.frame(SCHEMA, KagemushaAttestedNorito.payload {
        writeUnsigned(it)
        it.array(signature, 64)
    })

    /**
     * Verify the pinned identity: the root signature, the pinned scheme id, and that the scheme
     * id is derived from this descriptor's chain, asset, reserve and the root key.
     */
    fun verifyRoot(pinnedSchemeId: ByteArray, rootPublicKey: ByteArray): Boolean =
        scheme.contentEquals(pinnedSchemeId) &&
            KagemushaAttestedIds.schemeId(chainId, assetDefinitionId, reserveAccountId, rootPublicKey)
                .contentEquals(scheme) &&
            KagemushaAttestedCrypto.verify(
                rootPublicKey,
                KagemushaAttestedDomain.DESCRIPTOR.message(unsignedBytes()),
                signature,
            )

    /** Verify an issuer signature made by descriptor key [keyIndex] over `domain || unsigned`. */
    fun verifyIssuer(
        keyIndex: Int,
        domain: KagemushaAttestedDomain,
        unsigned: ByteArray,
        signature: ByteArray,
        atMs: Long?,
    ): Boolean {
        val key = issuerKey(keyIndex) ?: return false
        if (atMs != null && !key.validAt(atMs)) return false
        return KagemushaAttestedCrypto.verify(key.publicKey(), domain.message(unsigned), signature)
    }

    override fun equals(other: Any?): Boolean = other is KagemushaAttestedSchemeDescriptorV1 &&
        encode().contentEquals(other.encode())

    override fun hashCode(): Int = scheme.contentHashCode() * 31 + descriptorEpoch.hashCode()

    companion object {
        const val SCHEMA: String = MODEL + "KagemushaAttestedSchemeDescriptorV1"

        /** Build and root-sign a descriptor; used by issuers and test schemes. */
        @JvmStatic
        fun signed(
            unsigned: KagemushaAttestedSchemeDescriptorV1,
            rootSigner: KagemushaDeviceSigner,
        ): KagemushaAttestedSchemeDescriptorV1 {
            val signature = KagemushaAttestedCrypto.signAndCheck(
                rootSigner,
                rootSigner.publicKey(),
                KagemushaAttestedDomain.DESCRIPTOR.message(unsigned.unsignedBytes()),
            )
            return unsigned.withSignature(signature)
        }

        @JvmStatic
        fun decode(bytes: ByteArray): KagemushaAttestedSchemeDescriptorV1 {
            val payload = KagemushaAttestedNorito.unframe(bytes, SCHEMA, KagemushaAttestedLimits.DESCRIPTOR_BYTES)
            val value = KagemushaAttestedNorito.readPayload(payload) { input ->
                KagemushaAttestedSchemeDescriptorV1(
                    input.u16(),
                    input.array(32),
                    input.u64(),
                    input.string(KagemushaAttestedLimits.MAXIMUM_STRING_BYTES),
                    input.string(KagemushaAttestedLimits.MAXIMUM_STRING_BYTES),
                    input.u8(),
                    input.string(KagemushaAttestedLimits.MAXIMUM_STRING_BYTES),
                    input.string(KagemushaAttestedLimits.MAXIMUM_STRING_BYTES),
                    input.vec(KagemushaAttestedLimits.MAXIMUM_ISSUER_KEYS, KagemushaAttestedIssuerKeyV1::read),
                    input.opaque(KagemushaAttestedLimits.MAXIMUM_POLICY_BYTES),
                    input.opaque(KagemushaAttestedLimits.MAXIMUM_POLICY_BYTES),
                    input.vec(KagemushaAttestedLimits.MAXIMUM_TIERS, KagemushaAttestedTierV1::read),
                    input.nested(KagemushaAttestedAccountLimitsV1::read),
                    input.u64(),
                    input.u64(),
                    input.bool(),
                    input.array(64),
                )
            }
            require(value.encode().contentEquals(bytes)) { "KAGEMUSHA attested descriptor is not canonical" }
            return value
        }
    }

    internal fun withSignature(signature: ByteArray) = KagemushaAttestedSchemeDescriptorV1(
        version, scheme, descriptorEpoch, chainId, assetDefinitionId, assetScale, reserveAccountId, issuerUrl,
        issuerKeys, android, apple, tiers, accountLimits, receiverGraceMs, headroomQuantum, allowTestDevices, signature,
    )
}

/** Issuer-certified device certificate. It carries no account; the issuer keeps that mapping. */
class KagemushaAttestedDeviceCertV1(
    @JvmField val version: Int,
    schemeId: ByteArray,
    deviceId: ByteArray,
    devicePublicKey: ByteArray,
    @JvmField val platform: KagemushaAttestedPlatform,
    @JvmField val tier: Int,
    @JvmField val certSerial: Long,
    @JvmField val notBeforeMs: Long,
    @JvmField val notAfterMs: Long,
    @JvmField val maxBalance: Long,
    @JvmField val maxPayment: Long,
    @JvmField val maxUnsyncedOut: Long,
    @JvmField val issuerKeyIndex: Int,
    issuerSignature: ByteArray,
) {
    private val scheme = fixed(schemeId, 32, "scheme_id")
    private val device = fixed(deviceId, 32, "device_id")
    private val key = KagemushaAttestedCrypto.sec1(KagemushaAttestedCrypto.publicKey(devicePublicKey))
    private val signature = fixed(issuerSignature, 64, "issuer signature")

    init {
        require(version == KagemushaAttestedLimits.VERSION) { "unsupported certificate version" }
        requireU8(tier, "certificate tier")
        require(certSerial in 0..0xffff_ffffL) { "certificate serial is not a u32" }
        requireNonNegative(notBeforeMs, "certificate not_before_ms")
        require(notAfterMs >= notBeforeMs) { "certificate validity window is inverted" }
        requireAmount(maxBalance, "certificate max_balance")
        requireAmount(maxPayment, "certificate max_payment")
        requireAmount(maxUnsyncedOut, "certificate max_unsynced_out")
        requireU8(issuerKeyIndex, "certificate issuer key index")
        require(KagemushaAttestedIds.deviceId(scheme, key).contentEquals(device)) {
            "certificate device_id is not derived from its scheme and public key"
        }
    }

    fun schemeId(): ByteArray = scheme.copyOf()
    fun deviceId(): ByteArray = device.copyOf()
    fun devicePublicKey(): ByteArray = key.copyOf()
    fun issuerSignature(): ByteArray = signature.copyOf()

    internal fun sameDevice(other: ByteArray): Boolean = device.contentEquals(other)

    private fun writeUnsigned(out: NoritoOut) {
        out.u16(version)
        out.array(scheme, 32)
        out.array(device, 32)
        out.array(key, 65)
        out.u8(platform.code)
        out.u8(tier)
        out.u32(certSerial)
        out.u64(notBeforeMs)
        out.u64(notAfterMs)
        out.u64(maxBalance)
        out.u64(maxPayment)
        out.u64(maxUnsyncedOut)
        out.u8(issuerKeyIndex)
    }

    internal fun writeSigned(out: NoritoOut) {
        writeUnsigned(out)
        out.array(signature, 64)
    }

    fun unsignedBytes(): ByteArray = KagemushaAttestedNorito.frame(SCHEMA, KagemushaAttestedNorito.payload(::writeUnsigned))

    /** Certificate digest `H(D_cert || unsigned)`, the Bootstrap subject. */
    fun digest(): ByteArray = KagemushaAttestedDomain.CERT.hash(unsignedBytes())

    fun encode(): ByteArray = KagemushaAttestedNorito.frame(SCHEMA, KagemushaAttestedNorito.payload(::writeSigned))

    /** Verify the issuer signature, the scheme and that the signing key was valid at issuance. */
    fun verifyIssuer(descriptor: KagemushaAttestedSchemeDescriptorV1): Boolean =
        scheme.contentEquals(descriptor.schemeId()) &&
            descriptor.verifyIssuer(issuerKeyIndex, KagemushaAttestedDomain.CERT, unsignedBytes(), signature, notBeforeMs)

    override fun equals(other: Any?): Boolean = other is KagemushaAttestedDeviceCertV1 && encode().contentEquals(other.encode())

    override fun hashCode(): Int = device.contentHashCode() * 31 + certSerial.hashCode()

    companion object {
        const val SCHEMA: String = MODEL + "KagemushaAttestedDeviceCertV1"

        internal fun read(input: NoritoIn): KagemushaAttestedDeviceCertV1 = KagemushaAttestedDeviceCertV1(
            input.u16(),
            input.array(32),
            input.array(32),
            input.array(65),
            input.u8().let { KagemushaAttestedPlatform.fromCode(it) ?: throw IllegalArgumentException("unknown certificate platform") },
            input.u8(),
            input.u32(),
            input.u64(),
            input.u64(),
            input.u64(),
            input.u64(),
            input.u64(),
            input.u8(),
            input.array(64),
        )

        @JvmStatic
        fun decode(bytes: ByteArray): KagemushaAttestedDeviceCertV1 {
            val payload = KagemushaAttestedNorito.unframe(bytes, SCHEMA, KagemushaAttestedLimits.CERT_BYTES)
            val value = KagemushaAttestedNorito.readPayload(payload, ::read)
            require(value.encode().contentEquals(bytes)) { "KAGEMUSHA attested certificate is not canonical" }
            return value
        }

        /** Issue a certificate signed by [issuerSigner] under descriptor key [issuerKeyIndex]. */
        @JvmStatic
        fun issue(
            schemeId: ByteArray,
            devicePublicKey: ByteArray,
            platform: KagemushaAttestedPlatform,
            tier: KagemushaAttestedTierV1,
            certSerial: Long,
            notBeforeMs: Long,
            issuerKeyIndex: Int,
            issuerSigner: KagemushaDeviceSigner,
        ): KagemushaAttestedDeviceCertV1 {
            val unsigned = KagemushaAttestedDeviceCertV1(
                KagemushaAttestedLimits.VERSION, schemeId, KagemushaAttestedIds.deviceId(schemeId, devicePublicKey),
                devicePublicKey, platform, tier.tier, certSerial, notBeforeMs,
                Math.addExact(notBeforeMs, tier.leaseMs), tier.maxBalance, tier.maxPayment, tier.maxUnsyncedOut,
                issuerKeyIndex, ByteArray(64),
            )
            val signature = KagemushaAttestedCrypto.signAndCheck(
                issuerSigner,
                issuerSigner.publicKey(),
                KagemushaAttestedDomain.CERT.message(unsigned.unsignedBytes()),
            )
            return KagemushaAttestedDeviceCertV1(
                unsigned.version, unsigned.scheme, unsigned.device, unsigned.key, platform, unsigned.tier,
                certSerial, notBeforeMs, unsigned.notAfterMs, unsigned.maxBalance, unsigned.maxPayment,
                unsigned.maxUnsyncedOut, issuerKeyIndex, signature,
            )
        }
    }
}

/** One device state transition. It is always signed separately by the device key. */
class KagemushaAttestedTransitionV1(
    @JvmField val version: Int,
    deviceId: ByteArray,
    @JvmField val seq: Long,
    prevDigest: ByteArray,
    @JvmField val kind: KagemushaTransitionKind,
    @JvmField val amount: Long,
    @JvmField val balanceAfter: Long,
    subject: ByteArray,
    counterparty: ByteArray,
    @JvmField val ackedSeq: Long,
    @JvmField val unsyncedOutAfter: Long,
    @JvmField val crlEpochHeld: Long,
    @JvmField val deviceTimeMs: Long,
) {
    private val device = fixed(deviceId, 32, "device_id")
    private val prev = fixed(prevDigest, 32, "prev_digest")
    private val subjectBytes = fixed(subject, 32, "subject")
    private val counterpartyBytes = fixed(counterparty, 32, "counterparty")

    init {
        require(version == KagemushaAttestedLimits.VERSION) { "unsupported transition version" }
        requireNonNegative(seq, "seq")
        requireAmount(amount, "transition amount")
        requireAmount(balanceAfter, "transition balance_after")
        requireNonNegative(ackedSeq, "acked_seq")
        require(ackedSeq <= seq) { "acked_seq exceeds seq" }
        requireNonNegative(unsyncedOutAfter, "unsynced_out_after")
        requireNonNegative(crlEpochHeld, "crl_epoch_held")
        requireNonNegative(deviceTimeMs, "device_time_ms")
        when (kind) {
            KagemushaTransitionKind.BOOTSTRAP -> require(seq == 0L && prev.all { it.toInt() == 0 } && amount == 0L &&
                balanceAfter == 0L && unsyncedOutAfter == 0L && counterpartyBytes.all { it.toInt() == 0 }
            ) { "Bootstrap must be seq 0 with a zero predecessor, amount and balance" }
            KagemushaTransitionKind.SEND_SPLIT, KagemushaTransitionKind.RECEIVE_FOLD ->
                require(seq > 0 && amount > 0) { "peer transitions need seq > 0 and a positive amount" }
            KagemushaTransitionKind.MINT_FOLD, KagemushaTransitionKind.REDEEM_SPLIT -> require(
                seq > 0 && amount > 0 && counterpartyBytes.all { it.toInt() == 0 },
            ) { "mint and redeem transitions need a positive amount and no counterparty" }
            KagemushaTransitionKind.ROTATE -> require(seq > 0 && amount == 0L) { "Rotate moves no value" }
        }
    }

    fun deviceId(): ByteArray = device.copyOf()
    fun prevDigest(): ByteArray = prev.copyOf()
    fun subject(): ByteArray = subjectBytes.copyOf()
    fun counterparty(): ByteArray = counterpartyBytes.copyOf()

    internal fun write(out: NoritoOut) {
        out.u16(version)
        out.array(device, 32)
        out.u64(seq)
        out.array(prev, 32)
        out.u8(kind.code)
        out.u64(amount)
        out.u64(balanceAfter)
        out.array(subjectBytes, 32)
        out.array(counterpartyBytes, 32)
        out.u64(ackedSeq)
        out.u64(unsyncedOutAfter)
        out.u64(crlEpochHeld)
        out.u64(deviceTimeMs)
    }

    fun encode(): ByteArray = KagemushaAttestedNorito.frame(SCHEMA, KagemushaAttestedNorito.payload(::write))

    /** `digest(T) = H(D_transition || T)`; for a SendSplit this is the payment id and nullifier. */
    fun digest(): ByteArray = KagemushaAttestedDomain.TRANSITION.hash(encode())

    /** Exact message signed by the device key. */
    fun signingMessage(): ByteArray = KagemushaAttestedDomain.TRANSITION.message(encode())

    /** Verify a device signature for this transition. */
    fun verify(devicePublicKey: ByteArray, signature: ByteArray): Boolean =
        KagemushaAttestedCrypto.verify(devicePublicKey, signingMessage(), signature)

    override fun equals(other: Any?): Boolean = other is KagemushaAttestedTransitionV1 && encode().contentEquals(other.encode())

    override fun hashCode(): Int = device.contentHashCode() * 31 + seq.hashCode()

    companion object {
        const val SCHEMA: String = MODEL + "KagemushaAttestedTransitionV1"

        internal fun read(input: NoritoIn): KagemushaAttestedTransitionV1 = KagemushaAttestedTransitionV1(
            input.u16(),
            input.array(32),
            input.u64(),
            input.array(32),
            input.u8().let { KagemushaTransitionKind.fromCode(it) ?: throw IllegalArgumentException("unknown transition kind") },
            input.u64(),
            input.u64(),
            input.array(32),
            input.array(32),
            input.u64(),
            input.u64(),
            input.u64(),
            input.u64(),
        )

        @JvmStatic
        fun decode(bytes: ByteArray): KagemushaAttestedTransitionV1 {
            val payload = KagemushaAttestedNorito.unframe(bytes, SCHEMA, KagemushaAttestedLimits.TRANSITION_BYTES)
            val value = KagemushaAttestedNorito.readPayload(payload, ::read)
            require(value.encode().contentEquals(bytes)) { "KAGEMUSHA attested transition is not canonical" }
            return value
        }
    }
}

/** One revocation entry. */
class KagemushaAttestedRevocationEntryV1(
    deviceId: ByteArray,
    @JvmField val reason: KagemushaRevocationReason,
    @JvmField val epoch: Long,
) {
    private val device = fixed(deviceId, 32, "revoked device_id")

    init {
        requireNonNegative(epoch, "revocation epoch")
    }

    fun deviceId(): ByteArray = device.copyOf()

    internal fun matches(deviceId: ByteArray): Boolean = device.contentEquals(deviceId)

    internal fun write(out: NoritoOut) {
        out.array(device, 32)
        out.u8(reason.code)
        out.u64(epoch)
    }

    override fun equals(other: Any?): Boolean = other is KagemushaAttestedRevocationEntryV1 &&
        device.contentEquals(other.device) && reason == other.reason && epoch == other.epoch

    override fun hashCode(): Int = device.contentHashCode() * 31 + reason.hashCode()

    internal companion object {
        fun read(input: NoritoIn) = KagemushaAttestedRevocationEntryV1(
            input.array(32),
            input.u8().let { KagemushaRevocationReason.fromCode(it) ?: throw IllegalArgumentException("unknown revocation reason") },
            input.u64(),
        )
    }
}

/** Issuer-signed full revocation list at one epoch. */
class KagemushaAttestedRevocationListV1(
    schemeId: ByteArray,
    @JvmField val epoch: Long,
    @JvmField val issuedAtMs: Long,
    entries: List<KagemushaAttestedRevocationEntryV1>,
    @JvmField val keyIndex: Int,
    signature: ByteArray,
) {
    private val scheme = fixed(schemeId, 32, "scheme_id")
    private val sig = fixed(signature, 64, "CRL signature")

    @JvmField val entries: List<KagemushaAttestedRevocationEntryV1> = entries.toList()

    init {
        requireNonNegative(epoch, "CRL epoch")
        requireNonNegative(issuedAtMs, "CRL issued_at_ms")
        require(this.entries.size <= KagemushaAttestedLimits.MAXIMUM_CRL_ENTRIES) { "CRL is too large" }
        require(this.entries.all { it.epoch <= epoch }) { "CRL entry epoch exceeds the list epoch" }
        requireU8(keyIndex, "CRL key index")
    }

    fun schemeId(): ByteArray = scheme.copyOf()
    fun signature(): ByteArray = sig.copyOf()

    /** The entry revoking [deviceId], if any. */
    fun entryFor(deviceId: ByteArray): KagemushaAttestedRevocationEntryV1? = entries.firstOrNull { it.matches(deviceId) }

    private fun writeUnsigned(out: NoritoOut) {
        out.array(scheme, 32)
        out.u64(epoch)
        out.u64(issuedAtMs)
        out.vec(entries) { element, entry -> entry.write(element) }
        out.u8(keyIndex)
    }

    internal fun writeSigned(out: NoritoOut) {
        writeUnsigned(out)
        out.array(sig, 64)
    }

    fun unsignedBytes(): ByteArray = KagemushaAttestedNorito.frame(SCHEMA, KagemushaAttestedNorito.payload(::writeUnsigned))

    fun encode(): ByteArray = KagemushaAttestedNorito.frame(SCHEMA, KagemushaAttestedNorito.payload(::writeSigned))

    fun verify(descriptor: KagemushaAttestedSchemeDescriptorV1): Boolean =
        scheme.contentEquals(descriptor.schemeId()) &&
            descriptor.verifyIssuer(keyIndex, KagemushaAttestedDomain.CRL, unsignedBytes(), sig, issuedAtMs)

    override fun equals(other: Any?): Boolean = other is KagemushaAttestedRevocationListV1 && encode().contentEquals(other.encode())

    override fun hashCode(): Int = scheme.contentHashCode() * 31 + epoch.hashCode()

    companion object {
        const val SCHEMA: String = MODEL + "KagemushaAttestedRevocationListV1"

        internal fun read(input: NoritoIn) = KagemushaAttestedRevocationListV1(
            input.array(32),
            input.u64(),
            input.u64(),
            input.vec(KagemushaAttestedLimits.MAXIMUM_CRL_ENTRIES, KagemushaAttestedRevocationEntryV1::read),
            input.u8(),
            input.array(64),
        )

        @JvmStatic
        fun decode(bytes: ByteArray): KagemushaAttestedRevocationListV1 {
            val payload = KagemushaAttestedNorito.unframe(bytes, SCHEMA, KagemushaAttestedLimits.CRL_BYTES)
            val value = KagemushaAttestedNorito.readPayload(payload, ::read)
            require(value.encode().contentEquals(bytes)) { "KAGEMUSHA attested CRL is not canonical" }
            return value
        }

        @JvmStatic
        fun issue(
            schemeId: ByteArray,
            epoch: Long,
            issuedAtMs: Long,
            entries: List<KagemushaAttestedRevocationEntryV1>,
            keyIndex: Int,
            issuerSigner: KagemushaDeviceSigner,
        ): KagemushaAttestedRevocationListV1 {
            val unsigned = KagemushaAttestedRevocationListV1(schemeId, epoch, issuedAtMs, entries, keyIndex, ByteArray(64))
            val signature = KagemushaAttestedCrypto.signAndCheck(
                issuerSigner,
                issuerSigner.publicKey(),
                KagemushaAttestedDomain.CRL.message(unsigned.unsignedBytes()),
            )
            return KagemushaAttestedRevocationListV1(schemeId, epoch, issuedAtMs, entries, keyIndex, signature)
        }
    }
}

/** Issuer-signed CRL delta gossiped offline inside Requests and Payments. */
class KagemushaAttestedRevocationDeltaV1(
    @JvmField val fromEpoch: Long,
    @JvmField val toEpoch: Long,
    entries: List<KagemushaAttestedRevocationEntryV1>,
    @JvmField val keyIndex: Int,
    signature: ByteArray,
) {
    private val sig = fixed(signature, 64, "CRL delta signature")

    @JvmField val entries: List<KagemushaAttestedRevocationEntryV1> = entries.toList()

    init {
        requireNonNegative(fromEpoch, "delta from_epoch")
        require(toEpoch > fromEpoch) { "delta must advance the epoch" }
        require(this.entries.size <= KagemushaAttestedLimits.MAXIMUM_DELTA_ENTRIES) { "delta carries at most eight entries" }
        require(this.entries.all { it.epoch in (fromEpoch + 1)..toEpoch }) { "delta entry epoch is outside the delta" }
        requireU8(keyIndex, "delta key index")
    }

    fun signature(): ByteArray = sig.copyOf()

    internal fun writeUnsigned(out: NoritoOut) {
        out.u64(fromEpoch)
        out.u64(toEpoch)
        out.vec(entries) { element, entry -> entry.write(element) }
        out.u8(keyIndex)
    }

    internal fun writeSigned(out: NoritoOut) {
        writeUnsigned(out)
        out.array(sig, 64)
    }

    fun unsignedBytes(): ByteArray = KagemushaAttestedNorito.frame(SCHEMA, KagemushaAttestedNorito.payload(::writeUnsigned))

    fun encode(): ByteArray = KagemushaAttestedNorito.frame(SCHEMA, KagemushaAttestedNorito.payload(::writeSigned))

    fun verify(descriptor: KagemushaAttestedSchemeDescriptorV1): Boolean =
        descriptor.verifyIssuer(keyIndex, KagemushaAttestedDomain.CRL_DELTA, unsignedBytes(), sig, null)

    override fun equals(other: Any?): Boolean = other is KagemushaAttestedRevocationDeltaV1 && encode().contentEquals(other.encode())

    override fun hashCode(): Int = toEpoch.hashCode() * 31 + sig.contentHashCode()

    companion object {
        const val SCHEMA: String = MODEL + "KagemushaAttestedRevocationDeltaV1"

        internal fun read(input: NoritoIn) = KagemushaAttestedRevocationDeltaV1(
            input.u64(),
            input.u64(),
            input.vec(KagemushaAttestedLimits.MAXIMUM_DELTA_ENTRIES, KagemushaAttestedRevocationEntryV1::read),
            input.u8(),
            input.array(64),
        )

        @JvmStatic
        fun decode(bytes: ByteArray): KagemushaAttestedRevocationDeltaV1 {
            val payload = KagemushaAttestedNorito.unframe(bytes, SCHEMA, KagemushaAttestedLimits.DELTA_BYTES)
            val value = KagemushaAttestedNorito.readPayload(payload, ::read)
            require(value.encode().contentEquals(bytes)) { "KAGEMUSHA attested CRL delta is not canonical" }
            return value
        }

        @JvmStatic
        fun issue(
            fromEpoch: Long,
            toEpoch: Long,
            entries: List<KagemushaAttestedRevocationEntryV1>,
            keyIndex: Int,
            issuerSigner: KagemushaDeviceSigner,
        ): KagemushaAttestedRevocationDeltaV1 {
            val unsigned = KagemushaAttestedRevocationDeltaV1(fromEpoch, toEpoch, entries, keyIndex, ByteArray(64))
            val signature = KagemushaAttestedCrypto.signAndCheck(
                issuerSigner,
                issuerSigner.publicKey(),
                KagemushaAttestedDomain.CRL_DELTA.message(unsigned.unsignedBytes()),
            )
            return KagemushaAttestedRevocationDeltaV1(fromEpoch, toEpoch, entries, keyIndex, signature)
        }
    }
}

/** Receiver-signed payment request (IPM1 kind 1). */
class KagemushaAttestedPaymentRequestV1(
    @JvmField val version: Int,
    @JvmField val receiverCert: KagemushaAttestedDeviceCertV1,
    requestNonce: ByteArray,
    /** Requested amount in minor units; zero means the payer enters the amount. */
    @JvmField val amount: Long,
    /** Receiver headroom, quantized down to the descriptor `headroom_quantum`. */
    @JvmField val headroom: Long,
    @JvmField val reusable: Boolean,
    @JvmField val maxUses: Int,
    /** Advisory creation time. The payer never checks request age. */
    @JvmField val createdAtMs: Long,
    @JvmField val crlEpochHeld: Long,
    @JvmField val crlDelta: KagemushaAttestedRevocationDeltaV1?,
    signature: ByteArray,
) {
    private val nonce = fixed(requestNonce, 16, "request_nonce")
    private val sig = fixed(signature, 64, "request signature")

    init {
        require(version == KagemushaAttestedLimits.VERSION) { "unsupported request version" }
        requireAmount(amount, "request amount")
        requireAmount(headroom, "request headroom")
        require(maxUses in 1..0xffff && (reusable || maxUses == 1)) { "request max_uses is invalid" }
        requireNonNegative(createdAtMs, "request created_at_ms")
        requireNonNegative(crlEpochHeld, "request crl_epoch_held")
    }

    fun requestNonce(): ByteArray = nonce.copyOf()
    fun signature(): ByteArray = sig.copyOf()

    private fun writeUnsigned(out: NoritoOut) {
        out.u16(version)
        out.nested { receiverCert.writeSigned(it) }
        out.array(nonce, 16)
        out.u64(amount)
        out.u64(headroom)
        out.bool(reusable)
        out.u16(maxUses)
        out.u64(createdAtMs)
        out.u64(crlEpochHeld)
        out.option(crlDelta) { inner, delta -> delta.writeSigned(inner) }
    }

    fun unsignedBytes(): ByteArray = KagemushaAttestedNorito.frame(SCHEMA, KagemushaAttestedNorito.payload(::writeUnsigned))

    /** `request_digest = H(D_request || unsigned)`. */
    fun digest(): ByteArray = KagemushaAttestedDomain.REQUEST.hash(unsignedBytes())

    fun signingMessage(): ByteArray = KagemushaAttestedDomain.REQUEST.message(unsignedBytes())

    fun encode(): ByteArray = KagemushaAttestedNorito.frame(SCHEMA, KagemushaAttestedNorito.payload {
        writeUnsigned(it)
        it.array(sig, 64)
    })

    /** Verify the receiver signature under the embedded certificate key. */
    fun verifySignature(): Boolean = KagemushaAttestedCrypto.verify(receiverCert.devicePublicKey(), signingMessage(), sig)

    override fun equals(other: Any?): Boolean = other is KagemushaAttestedPaymentRequestV1 && encode().contentEquals(other.encode())

    override fun hashCode(): Int = nonce.contentHashCode()

    companion object {
        const val SCHEMA: String = MODEL + "KagemushaAttestedPaymentRequestV1"

        @JvmStatic
        fun decode(bytes: ByteArray): KagemushaAttestedPaymentRequestV1 {
            val payload = KagemushaAttestedNorito.unframe(bytes, SCHEMA, KagemushaAttestedLimits.REQUEST_BYTES)
            val value = KagemushaAttestedNorito.readPayload(payload) { input ->
                KagemushaAttestedPaymentRequestV1(
                    input.u16(),
                    input.nested(KagemushaAttestedDeviceCertV1::read),
                    input.array(16),
                    input.u64(),
                    input.u64(),
                    input.bool(),
                    input.u16(),
                    input.u64(),
                    input.u64(),
                    input.option(KagemushaAttestedRevocationDeltaV1::read),
                    input.array(64),
                )
            }
            require(value.encode().contentEquals(bytes)) { "KAGEMUSHA attested request is not canonical" }
            return value
        }

        internal fun unsignedFor(
            receiverCert: KagemushaAttestedDeviceCertV1,
            requestNonce: ByteArray,
            amount: Long,
            headroom: Long,
            reusable: Boolean,
            maxUses: Int,
            createdAtMs: Long,
            crlEpochHeld: Long,
            crlDelta: KagemushaAttestedRevocationDeltaV1?,
        ) = KagemushaAttestedPaymentRequestV1(
            KagemushaAttestedLimits.VERSION, receiverCert, requestNonce, amount, headroom, reusable, maxUses,
            createdAtMs, crlEpochHeld, crlDelta, ByteArray(64),
        )
    }

    internal fun withSignature(signature: ByteArray) = KagemushaAttestedPaymentRequestV1(
        version, receiverCert, nonce, amount, headroom, reusable, maxUses, createdAtMs, crlEpochHeld, crlDelta, signature,
    )
}

/** Payer-signed payment (IPM1 kind 2): the payer certificate and its signed SendSplit. */
class KagemushaAttestedPaymentV1(
    @JvmField val version: Int,
    @JvmField val payerCert: KagemushaAttestedDeviceCertV1,
    @JvmField val transition: KagemushaAttestedTransitionV1,
    signature: ByteArray,
    @JvmField val crlDelta: KagemushaAttestedRevocationDeltaV1?,
) {
    private val sig = fixed(signature, 64, "payment signature")

    init {
        require(version == KagemushaAttestedLimits.VERSION) { "unsupported payment version" }
    }

    fun signature(): ByteArray = sig.copyOf()

    /** `payment_id = digest(SendSplit)`; also the nullifier. */
    fun paymentId(): ByteArray = transition.digest()

    internal fun write(out: NoritoOut) {
        out.u16(version)
        out.nested { payerCert.writeSigned(it) }
        out.nested { transition.write(it) }
        out.array(sig, 64)
        out.option(crlDelta) { inner, delta -> delta.writeSigned(inner) }
    }

    fun encode(): ByteArray = KagemushaAttestedNorito.frame(SCHEMA, KagemushaAttestedNorito.payload(::write))

    /** True when the signature verifies under the payer certificate key. */
    fun verifySignature(): Boolean = transition.verify(payerCert.devicePublicKey(), sig)

    override fun equals(other: Any?): Boolean = other is KagemushaAttestedPaymentV1 && encode().contentEquals(other.encode())

    override fun hashCode(): Int = transition.hashCode()

    companion object {
        const val SCHEMA: String = MODEL + "KagemushaAttestedPaymentV1"

        internal fun read(input: NoritoIn) = KagemushaAttestedPaymentV1(
            input.u16(),
            input.nested(KagemushaAttestedDeviceCertV1::read),
            input.nested(KagemushaAttestedTransitionV1::read),
            input.array(64),
            input.option(KagemushaAttestedRevocationDeltaV1::read),
        )

        @JvmStatic
        fun decode(bytes: ByteArray): KagemushaAttestedPaymentV1 {
            val payload = KagemushaAttestedNorito.unframe(bytes, SCHEMA, KagemushaAttestedLimits.PAYMENT_BYTES)
            val value = KagemushaAttestedNorito.readPayload(payload, ::read)
            require(value.encode().contentEquals(bytes)) { "KAGEMUSHA attested payment is not canonical" }
            return value
        }
    }
}

/** Receiver-signed acknowledgement (IPM1 kind 3). A courtesy only; it never conditions finality. */
class KagemushaAttestedAcknowledgementV1(
    @JvmField val version: Int,
    paymentId: ByteArray,
    receiverDeviceId: ByteArray,
    receiveTransitionDigest: ByteArray,
    signature: ByteArray,
) {
    private val payment = fixed(paymentId, 32, "payment_id")
    private val receiver = fixed(receiverDeviceId, 32, "receiver_device_id")
    private val receive = fixed(receiveTransitionDigest, 32, "receive_transition_digest")
    private val sig = fixed(signature, 64, "acknowledgement signature")

    init {
        require(version == KagemushaAttestedLimits.VERSION) { "unsupported acknowledgement version" }
    }

    fun paymentId(): ByteArray = payment.copyOf()
    fun receiverDeviceId(): ByteArray = receiver.copyOf()
    fun receiveTransitionDigest(): ByteArray = receive.copyOf()
    fun signature(): ByteArray = sig.copyOf()

    private fun writeUnsigned(out: NoritoOut) {
        out.u16(version)
        out.array(payment, 32)
        out.array(receiver, 32)
        out.array(receive, 32)
    }

    fun unsignedBytes(): ByteArray = KagemushaAttestedNorito.frame(SCHEMA, KagemushaAttestedNorito.payload(::writeUnsigned))

    fun signingMessage(): ByteArray = KagemushaAttestedDomain.ACK.message(unsignedBytes())

    fun encode(): ByteArray = KagemushaAttestedNorito.frame(SCHEMA, KagemushaAttestedNorito.payload {
        writeUnsigned(it)
        it.array(sig, 64)
    })

    fun verify(receiverPublicKey: ByteArray): Boolean = KagemushaAttestedCrypto.verify(receiverPublicKey, signingMessage(), sig)

    override fun equals(other: Any?): Boolean = other is KagemushaAttestedAcknowledgementV1 && encode().contentEquals(other.encode())

    override fun hashCode(): Int = payment.contentHashCode()

    companion object {
        const val SCHEMA: String = MODEL + "KagemushaAttestedAcknowledgementV1"

        @JvmStatic
        fun decode(bytes: ByteArray): KagemushaAttestedAcknowledgementV1 {
            val payload = KagemushaAttestedNorito.unframe(bytes, SCHEMA, KagemushaAttestedLimits.ACK_BYTES)
            val value = KagemushaAttestedNorito.readPayload(payload) { input ->
                KagemushaAttestedAcknowledgementV1(input.u16(), input.array(32), input.array(32), input.array(32), input.array(64))
            }
            require(value.encode().contentEquals(bytes)) { "KAGEMUSHA attested acknowledgement is not canonical" }
            return value
        }
    }
}

/** Issuer-signed mint voucher for one committed reserve load. */
class KagemushaAttestedMintVoucherV1(
    schemeId: ByteArray,
    voucherId: ByteArray,
    deviceId: ByteArray,
    loadId: ByteArray,
    @JvmField val amount: Long,
    txHash: ByteArray,
    @JvmField val issuedAtMs: Long,
    @JvmField val issuerKeyIndex: Int,
    signature: ByteArray,
) {
    private val scheme = fixed(schemeId, 32, "scheme_id")
    private val voucher = fixed(voucherId, 32, "voucher_id")
    private val device = fixed(deviceId, 32, "device_id")
    private val load = fixed(loadId, 16, "load_id")
    private val tx = fixed(txHash, 32, "tx_hash")
    private val sig = fixed(signature, 64, "voucher signature")

    init {
        require(amount in 1..KagemushaAmount.MAXIMUM_MINOR) { "voucher amount must be positive" }
        requireNonNegative(issuedAtMs, "voucher issued_at_ms")
        requireU8(issuerKeyIndex, "voucher key index")
        require(KagemushaAttestedIds.voucherId(tx, load).contentEquals(voucher)) {
            "voucher_id is not derived from tx_hash and load_id"
        }
    }

    fun schemeId(): ByteArray = scheme.copyOf()
    fun voucherId(): ByteArray = voucher.copyOf()
    fun deviceId(): ByteArray = device.copyOf()
    fun loadId(): ByteArray = load.copyOf()
    fun txHash(): ByteArray = tx.copyOf()
    fun signature(): ByteArray = sig.copyOf()

    private fun writeUnsigned(out: NoritoOut) {
        out.array(scheme, 32)
        out.array(voucher, 32)
        out.array(device, 32)
        out.array(load, 16)
        out.u64(amount)
        out.array(tx, 32)
        out.u64(issuedAtMs)
        out.u8(issuerKeyIndex)
    }

    fun unsignedBytes(): ByteArray = KagemushaAttestedNorito.frame(SCHEMA, KagemushaAttestedNorito.payload(::writeUnsigned))

    fun encode(): ByteArray = KagemushaAttestedNorito.frame(SCHEMA, KagemushaAttestedNorito.payload {
        writeUnsigned(it)
        it.array(sig, 64)
    })

    fun verify(descriptor: KagemushaAttestedSchemeDescriptorV1): Boolean =
        scheme.contentEquals(descriptor.schemeId()) &&
            descriptor.verifyIssuer(issuerKeyIndex, KagemushaAttestedDomain.VOUCHER, unsignedBytes(), sig, issuedAtMs)

    override fun equals(other: Any?): Boolean = other is KagemushaAttestedMintVoucherV1 && encode().contentEquals(other.encode())

    override fun hashCode(): Int = voucher.contentHashCode()

    companion object {
        const val SCHEMA: String = MODEL + "KagemushaAttestedMintVoucherV1"

        @JvmStatic
        fun decode(bytes: ByteArray): KagemushaAttestedMintVoucherV1 {
            val payload = KagemushaAttestedNorito.unframe(bytes, SCHEMA, KagemushaAttestedLimits.VOUCHER_BYTES)
            val value = KagemushaAttestedNorito.readPayload(payload) { input ->
                KagemushaAttestedMintVoucherV1(
                    input.array(32), input.array(32), input.array(32), input.array(16), input.u64(),
                    input.array(32), input.u64(), input.u8(), input.array(64),
                )
            }
            require(value.encode().contentEquals(bytes)) { "KAGEMUSHA attested voucher is not canonical" }
            return value
        }

        @JvmStatic
        fun issue(
            schemeId: ByteArray,
            deviceId: ByteArray,
            loadId: ByteArray,
            amount: Long,
            txHash: ByteArray,
            issuedAtMs: Long,
            issuerKeyIndex: Int,
            issuerSigner: KagemushaDeviceSigner,
        ): KagemushaAttestedMintVoucherV1 {
            val voucherId = KagemushaAttestedIds.voucherId(txHash, loadId)
            val unsigned = KagemushaAttestedMintVoucherV1(
                schemeId, voucherId, deviceId, loadId, amount, txHash, issuedAtMs, issuerKeyIndex, ByteArray(64),
            )
            val signature = KagemushaAttestedCrypto.signAndCheck(
                issuerSigner,
                issuerSigner.publicKey(),
                KagemushaAttestedDomain.VOUCHER.message(unsigned.unsignedBytes()),
            )
            return KagemushaAttestedMintVoucherV1(
                schemeId, voucherId, deviceId, loadId, amount, txHash, issuedAtMs, issuerKeyIndex, signature,
            )
        }
    }
}

/** Issuer authorization for a receiver to fold one payment it refused or never scanned. */
class KagemushaAttestedDeliveryV1(
    paymentId: ByteArray,
    receiverDeviceId: ByteArray,
    @JvmField val keyIndex: Int,
    signature: ByteArray,
) {
    private val payment = fixed(paymentId, 32, "payment_id")
    private val receiver = fixed(receiverDeviceId, 32, "receiver_device_id")
    private val sig = fixed(signature, 64, "delivery signature")

    init {
        requireU8(keyIndex, "delivery key index")
    }

    fun paymentId(): ByteArray = payment.copyOf()
    fun receiverDeviceId(): ByteArray = receiver.copyOf()
    fun signature(): ByteArray = sig.copyOf()

    internal fun writeUnsigned(out: NoritoOut) {
        out.array(payment, 32)
        out.array(receiver, 32)
        out.u8(keyIndex)
    }

    internal fun writeSigned(out: NoritoOut) {
        writeUnsigned(out)
        out.array(sig, 64)
    }

    fun unsignedBytes(): ByteArray = KagemushaAttestedNorito.frame(SCHEMA, KagemushaAttestedNorito.payload(::writeUnsigned))

    fun encode(): ByteArray = KagemushaAttestedNorito.frame(SCHEMA, KagemushaAttestedNorito.payload(::writeSigned))

    fun verify(descriptor: KagemushaAttestedSchemeDescriptorV1): Boolean =
        descriptor.verifyIssuer(keyIndex, KagemushaAttestedDomain.DELIVERY, unsignedBytes(), sig, null)

    override fun equals(other: Any?): Boolean = other is KagemushaAttestedDeliveryV1 && encode().contentEquals(other.encode())

    override fun hashCode(): Int = payment.contentHashCode()

    companion object {
        const val SCHEMA: String = MODEL + "KagemushaAttestedDeliveryV1"

        internal fun read(input: NoritoIn) =
            KagemushaAttestedDeliveryV1(input.array(32), input.array(32), input.u8(), input.array(64))

        @JvmStatic
        fun decode(bytes: ByteArray): KagemushaAttestedDeliveryV1 {
            val payload = KagemushaAttestedNorito.unframe(bytes, SCHEMA, KagemushaAttestedLimits.DELIVERY_BYTES)
            val value = KagemushaAttestedNorito.readPayload(payload, ::read)
            require(value.encode().contentEquals(bytes)) { "KAGEMUSHA attested delivery is not canonical" }
            return value
        }

        @JvmStatic
        fun issue(paymentId: ByteArray, receiverDeviceId: ByteArray, keyIndex: Int, issuerSigner: KagemushaDeviceSigner): KagemushaAttestedDeliveryV1 {
            val unsigned = KagemushaAttestedDeliveryV1(paymentId, receiverDeviceId, keyIndex, ByteArray(64))
            val signature = KagemushaAttestedCrypto.signAndCheck(
                issuerSigner,
                issuerSigner.publicKey(),
                KagemushaAttestedDomain.DELIVERY.message(unsigned.unsignedBytes()),
            )
            return KagemushaAttestedDeliveryV1(paymentId, receiverDeviceId, keyIndex, signature)
        }
    }
}

/** Fork evidence variants. */
enum class KagemushaForkEvidenceKind(val tag: Int) {
    /** Two distinct transition digests at the same `(device_id, seq)`. */
    SAME_SEQ(0),

    /**
     * Two sends with the same `acked_seq`, `seq_a < seq_b`, and
     * `unsynced_out_after_b < unsynced_out_after_a + amount_b`; no genuine app produces this.
     */
    INCONSISTENT(1),
}

/**
 * Self-contained evidence that one certified device signed two conflicting transitions.
 * Anyone holding the scheme descriptor can verify it.
 */
class KagemushaAttestedForkEvidenceV1(
    @JvmField val kind: KagemushaForkEvidenceKind,
    @JvmField val cert: KagemushaAttestedDeviceCertV1,
    @JvmField val a: KagemushaAttestedTransitionV1,
    signatureA: ByteArray,
    @JvmField val b: KagemushaAttestedTransitionV1,
    signatureB: ByteArray,
) {
    private val sigA = fixed(signatureA, 64, "evidence signature a")
    private val sigB = fixed(signatureB, 64, "evidence signature b")

    fun signatureA(): ByteArray = sigA.copyOf()
    fun signatureB(): ByteArray = sigB.copyOf()

    fun encode(): ByteArray = KagemushaAttestedNorito.frame(SCHEMA, KagemushaAttestedNorito.payload { out ->
        out.le(kind.tag.toLong(), 4)
        out.nested { cert.writeSigned(it) }
        out.nested { a.write(it) }
        out.array(sigA, 64)
        out.nested { b.write(it) }
        out.array(sigB, 64)
    })

    /** Verify the certificate, both signatures and the conflict rule; nothing else is needed. */
    fun verify(descriptor: KagemushaAttestedSchemeDescriptorV1): Boolean {
        if (!cert.verifyIssuer(descriptor)) return false
        if (!cert.sameDevice(a.deviceId()) || !cert.sameDevice(b.deviceId())) return false
        if (!a.verify(cert.devicePublicKey(), sigA) || !b.verify(cert.devicePublicKey(), sigB)) return false
        return when (kind) {
            KagemushaForkEvidenceKind.SAME_SEQ -> a.seq == b.seq && !a.digest().contentEquals(b.digest())
            KagemushaForkEvidenceKind.INCONSISTENT -> KagemushaAttestedForkRules.inconsistent(a, b)
        }
    }

    override fun equals(other: Any?): Boolean = other is KagemushaAttestedForkEvidenceV1 && encode().contentEquals(other.encode())

    override fun hashCode(): Int = a.hashCode() * 31 + b.hashCode()

    companion object {
        const val SCHEMA: String = MODEL + "KagemushaAttestedForkEvidenceV1"

        @JvmStatic
        fun decode(bytes: ByteArray): KagemushaAttestedForkEvidenceV1 {
            val payload = KagemushaAttestedNorito.unframe(bytes, SCHEMA, KagemushaAttestedLimits.FORK_EVIDENCE_BYTES)
            val value = KagemushaAttestedNorito.readPayload(payload) { input ->
                val tag = input.le(4).toInt()
                val kind = KagemushaForkEvidenceKind.entries.firstOrNull { it.tag == tag }
                    ?: throw IllegalArgumentException("unknown fork evidence variant")
                KagemushaAttestedForkEvidenceV1(
                    kind,
                    input.nested(KagemushaAttestedDeviceCertV1::read),
                    input.nested(KagemushaAttestedTransitionV1::read),
                    input.array(64),
                    input.nested(KagemushaAttestedTransitionV1::read),
                    input.array(64),
                )
            }
            require(value.encode().contentEquals(bytes)) { "KAGEMUSHA attested fork evidence is not canonical" }
            return value
        }
    }
}

/** Offline conflict rules shared by receivers and evidence verifiers. */
object KagemushaAttestedForkRules {
    /**
     * True when two SendSplits of one device are inconsistent: same `acked_seq`, and with
     * `a` the lower sequence, `unsynced_out_after_b < unsynced_out_after_a + amount_b`.
     */
    @JvmStatic
    fun inconsistent(first: KagemushaAttestedTransitionV1, second: KagemushaAttestedTransitionV1): Boolean {
        if (first.kind != KagemushaTransitionKind.SEND_SPLIT || second.kind != KagemushaTransitionKind.SEND_SPLIT) return false
        if (!first.deviceId().contentEquals(second.deviceId()) || first.ackedSeq != second.ackedSeq) return false
        if (first.seq == second.seq) return false
        val (a, b) = if (first.seq < second.seq) first to second else second to first
        return b.unsyncedOutAfter < a.unsyncedOutAfter + b.amount
    }
}

/** A receipt delivery: the issuer authorization plus the payer payment it authorizes. */
class KagemushaAttestedReceiptDeliveryV1(
    @JvmField val delivery: KagemushaAttestedDeliveryV1,
    @JvmField val payment: KagemushaAttestedPaymentV1,
) {
    init {
        require(delivery.paymentId().contentEquals(payment.paymentId())) { "delivery does not authorize this payment" }
    }

    internal fun write(out: NoritoOut) {
        out.nested { delivery.writeSigned(it) }
        out.nested { payment.write(it) }
    }

    override fun equals(other: Any?): Boolean = other is KagemushaAttestedReceiptDeliveryV1 &&
        delivery == other.delivery && payment == other.payment

    override fun hashCode(): Int = delivery.hashCode()

    internal companion object {
        fun read(input: NoritoIn) = KagemushaAttestedReceiptDeliveryV1(
            input.nested(KagemushaAttestedDeliveryV1::read),
            input.nested(KagemushaAttestedPaymentV1::read),
        )
    }
}

/** Issuer-side state of one redemption. */
class KagemushaAttestedRedemptionStatusV1(
    redemptionId: ByteArray,
    @JvmField val state: KagemushaRedemptionState,
    txHash: ByteArray?,
) {
    private val redemption = fixed(redemptionId, 32, "redemption_id")
    private val tx = txHash?.let { fixed(it, 32, "redemption tx_hash") }

    init {
        require((state == KagemushaRedemptionState.PAID) == (tx != null)) { "only a paid redemption carries a tx hash" }
    }

    fun redemptionId(): ByteArray = redemption.copyOf()
    fun txHash(): ByteArray? = tx?.copyOf()

    internal fun write(out: NoritoOut) {
        out.array(redemption, 32)
        out.u8(state.code)
        out.option(tx) { inner, hash -> inner.bareByteArray(hash) }
    }

    override fun equals(other: Any?): Boolean = other is KagemushaAttestedRedemptionStatusV1 &&
        redemption.contentEquals(other.redemption) && state == other.state && java.util.Arrays.equals(tx, other.tx)

    override fun hashCode(): Int = redemption.contentHashCode()

    internal companion object {
        fun read(input: NoritoIn) = KagemushaAttestedRedemptionStatusV1(
            input.array(32),
            input.u8().let { KagemushaRedemptionState.fromCode(it) ?: throw IllegalArgumentException("unknown redemption state") },
            input.option { it.bareByteArray(32) },
        )
    }
}

/** Issuer-signed sync receipt. Sync never confirms, holds or reverses a peer payment. */
class KagemushaAttestedSyncReceiptV1(
    deviceId: ByteArray,
    @JvmField val ackedSeq: Long,
    ackedDigest: ByteArray,
    @JvmField val renewedCert: KagemushaAttestedDeviceCertV1?,
    @JvmField val crl: KagemushaAttestedRevocationListV1,
    deliveries: List<KagemushaAttestedReceiptDeliveryV1>,
    redemptions: List<KagemushaAttestedRedemptionStatusV1>,
    nextSyncNonce: ByteArray,
    @JvmField val issuedAtMs: Long,
    @JvmField val keyIndex: Int,
    signature: ByteArray,
) {
    private val device = fixed(deviceId, 32, "device_id")
    private val acked = fixed(ackedDigest, 32, "acked_digest")
    private val nonce = fixed(nextSyncNonce, 32, "next_sync_nonce")
    private val sig = fixed(signature, 64, "receipt signature")

    @JvmField val deliveries: List<KagemushaAttestedReceiptDeliveryV1> = deliveries.toList()
    @JvmField val redemptions: List<KagemushaAttestedRedemptionStatusV1> = redemptions.toList()

    init {
        requireNonNegative(ackedSeq, "receipt acked_seq")
        require(this.deliveries.size <= KagemushaAttestedLimits.MAXIMUM_RECEIPT_DELIVERIES &&
            this.redemptions.size <= KagemushaAttestedLimits.MAXIMUM_RECEIPT_REDEMPTIONS
        ) { "receipt is too large" }
        requireNonNegative(issuedAtMs, "receipt issued_at_ms")
        requireU8(keyIndex, "receipt key index")
    }

    fun deviceId(): ByteArray = device.copyOf()
    fun ackedDigest(): ByteArray = acked.copyOf()
    fun nextSyncNonce(): ByteArray = nonce.copyOf()
    fun signature(): ByteArray = sig.copyOf()

    private fun writeUnsigned(out: NoritoOut) {
        out.array(device, 32)
        out.u64(ackedSeq)
        out.array(acked, 32)
        out.option(renewedCert) { inner, cert -> cert.writeSigned(inner) }
        out.nested { crl.writeSigned(it) }
        out.vec(deliveries) { element, delivery -> delivery.write(element) }
        out.vec(redemptions) { element, status -> status.write(element) }
        out.array(nonce, 32)
        out.u64(issuedAtMs)
        out.u8(keyIndex)
    }

    fun unsignedBytes(): ByteArray = KagemushaAttestedNorito.frame(SCHEMA, KagemushaAttestedNorito.payload(::writeUnsigned))

    fun encode(): ByteArray = KagemushaAttestedNorito.frame(SCHEMA, KagemushaAttestedNorito.payload {
        writeUnsigned(it)
        it.array(sig, 64)
    })

    fun verify(descriptor: KagemushaAttestedSchemeDescriptorV1): Boolean =
        descriptor.verifyIssuer(keyIndex, KagemushaAttestedDomain.SYNC, unsignedBytes(), sig, issuedAtMs)

    override fun equals(other: Any?): Boolean = other is KagemushaAttestedSyncReceiptV1 && encode().contentEquals(other.encode())

    override fun hashCode(): Int = device.contentHashCode() * 31 + ackedSeq.hashCode()

    companion object {
        const val SCHEMA: String = MODEL + "KagemushaAttestedSyncReceiptV1"

        @JvmStatic
        fun decode(bytes: ByteArray): KagemushaAttestedSyncReceiptV1 {
            val payload = KagemushaAttestedNorito.unframe(bytes, SCHEMA, KagemushaAttestedLimits.RECEIPT_BYTES)
            val value = KagemushaAttestedNorito.readPayload(payload) { input ->
                KagemushaAttestedSyncReceiptV1(
                    input.array(32),
                    input.u64(),
                    input.array(32),
                    input.option(KagemushaAttestedDeviceCertV1::read),
                    input.nested(KagemushaAttestedRevocationListV1::read),
                    input.vec(KagemushaAttestedLimits.MAXIMUM_RECEIPT_DELIVERIES, KagemushaAttestedReceiptDeliveryV1::read),
                    input.vec(KagemushaAttestedLimits.MAXIMUM_RECEIPT_REDEMPTIONS, KagemushaAttestedRedemptionStatusV1::read),
                    input.array(32),
                    input.u64(),
                    input.u8(),
                    input.array(64),
                )
            }
            require(value.encode().contentEquals(bytes)) { "KAGEMUSHA attested sync receipt is not canonical" }
            return value
        }

        @JvmStatic
        fun issue(
            deviceId: ByteArray,
            ackedSeq: Long,
            ackedDigest: ByteArray,
            renewedCert: KagemushaAttestedDeviceCertV1?,
            crl: KagemushaAttestedRevocationListV1,
            deliveries: List<KagemushaAttestedReceiptDeliveryV1>,
            redemptions: List<KagemushaAttestedRedemptionStatusV1>,
            nextSyncNonce: ByteArray,
            issuedAtMs: Long,
            keyIndex: Int,
            issuerSigner: KagemushaDeviceSigner,
        ): KagemushaAttestedSyncReceiptV1 {
            val unsigned = KagemushaAttestedSyncReceiptV1(
                deviceId, ackedSeq, ackedDigest, renewedCert, crl, deliveries, redemptions, nextSyncNonce,
                issuedAtMs, keyIndex, ByteArray(64),
            )
            val signature = KagemushaAttestedCrypto.signAndCheck(
                issuerSigner,
                issuerSigner.publicKey(),
                KagemushaAttestedDomain.SYNC.message(unsigned.unsignedBytes()),
            )
            return KagemushaAttestedSyncReceiptV1(
                deviceId, ackedSeq, ackedDigest, renewedCert, crl, deliveries, redemptions, nextSyncNonce,
                issuedAtMs, keyIndex, signature,
            )
        }
    }
}
