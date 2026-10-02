// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.attested

/** Hardware or evidence failure that prevents KAGEMUSHA value admission. */
enum class KagemushaUnsupportedReason {
    /** No TEE, StrongBox or Secure Enclave P-256 key could be created. */
    NO_HARDWARE_KEY,

    /** The scheme requires a platform integrity verdict the device cannot produce. */
    NO_PLAY_INTEGRITY,

    /** The issuer rejected the vendor attestation (root, boot state, package, signer). */
    ATTESTATION_REJECTED,

    /** The installed app version is below the scheme minimum. */
    BELOW_MIN_VERSION,

    /** The operating system or device class is outside the scheme policy. */
    UNSUPPORTED_PLATFORM,

    /** The descriptor allows test devices but the SDK is not in debug testing mode. */
    TEST_SCHEME_REFUSED,
}

/** Why a wallet is frozen. Recovery from a lost state or key is online only. */
sealed class KagemushaFrozenReason {
    /** The device key exists but the wallet state is missing or its head does not match. */
    object StateLost : KagemushaFrozenReason() {
        override fun toString(): String = "StateLost"
    }

    /** The wallet state exists but its device key is missing or replaced. */
    object KeyLost : KagemushaFrozenReason() {
        override fun toString(): String = "KeyLost"
    }

    /**
     * The issuer revoked this device. Every reason blocks paying and requesting. A device revoked
     * for Integrity, Superseded, Closed or Lost may still sync and redeem to its enrolled
     * account; a Fraud revocation also freezes redemptions. Receiving always remains possible.
     */
    class Revoked(@JvmField val reason: KagemushaRevocationReason) : KagemushaFrozenReason() {
        override fun equals(other: Any?): Boolean = other is Revoked && other.reason == reason
        override fun hashCode(): Int = reason.hashCode()
        override fun toString(): String = "Revoked($reason)"
    }
}

/** Honest wallet status. There is never a software-key fallback. */
sealed interface KagemushaStatus {
    /** Offline value is unavailable on this installation. */
    class Unsupported(@JvmField val reason: KagemushaUnsupportedReason) : KagemushaStatus {
        override fun equals(other: Any?): Boolean = other is Unsupported && other.reason == reason
        override fun hashCode(): Int = reason.hashCode()
        override fun toString(): String = "Unsupported($reason)"
    }

    /** No certificate has been issued to this installation yet. */
    object NotEnrolled : KagemushaStatus {
        override fun toString(): String = "NotEnrolled"
    }

    /**
     * Enrolled and usable. [leaseEndsAtMs] is the certificate expiry: paying stops after it
     * until the next sync, receiving never stops. [clockAnomaly] is advisory only.
     */
    class Ready(
        @JvmField val balance: KagemushaAmount,
        @JvmField val tier: Int,
        @JvmField val leaseEndsAtMs: Long,
        @JvmField val needsSync: Boolean,
        @JvmField val clockAnomaly: Boolean,
    ) : KagemushaStatus {
        override fun equals(other: Any?): Boolean = other is Ready && other.balance == balance &&
            other.tier == tier && other.leaseEndsAtMs == leaseEndsAtMs && other.needsSync == needsSync &&
            other.clockAnomaly == clockAnomaly

        override fun hashCode(): Int = balance.hashCode() * 31 + leaseEndsAtMs.hashCode()

        override fun toString(): String =
            "Ready(balance=${balance.minor}, tier=$tier, leaseEndsAtMs=$leaseEndsAtMs, needsSync=$needsSync, clockAnomaly=$clockAnomaly)"
    }

    /** The wallet cannot pay; see [KagemushaFrozenReason] for what remains possible. */
    class Frozen(@JvmField val reason: KagemushaFrozenReason) : KagemushaStatus {
        override fun equals(other: Any?): Boolean = other is Frozen && other.reason == reason
        override fun hashCode(): Int = reason.hashCode()
        override fun toString(): String = "Frozen($reason)"
    }
}

/** Which limit a refused operation would exceed. */
enum class KagemushaLimit {
    /** The certificate `max_payment`. */
    MAX_PAYMENT,

    /** The certificate `max_unsynced_out`. */
    MAX_UNSYNCED_OUT,

    /** The receiver's advertised headroom (its `max_balance` minus balance and reservations). */
    RECEIVER_HEADROOM,

    /** The number of simultaneously open payment requests. */
    OPEN_REQUESTS,

    /** The protocol amount range `0 < amount <= 10^15`. */
    AMOUNT_RANGE,
}

/** Typed SDK failures. Every verb either completes durably or throws one of these. */
sealed class KagemushaException(message: String, cause: Throwable? = null) : Exception(message, cause) {
    class NotEnrolled : KagemushaException("KAGEMUSHA wallet is not enrolled")

    class LeaseExpired(@JvmField val leaseEndedAtMs: Long) :
        KagemushaException("KAGEMUSHA certificate lease ended; sync before paying")

    class InsufficientBalance(@JvmField val balance: KagemushaAmount, @JvmField val requested: KagemushaAmount) :
        KagemushaException("KAGEMUSHA balance is insufficient")

    class LimitExceeded(@JvmField val which: KagemushaLimit) :
        KagemushaException("KAGEMUSHA limit exceeded: $which")

    class InvalidRequest(detail: String, cause: Throwable? = null) :
        KagemushaException("KAGEMUSHA request is invalid: $detail", cause)

    class WrongReceiver : KagemushaException("KAGEMUSHA message is addressed to another device")

    class Revoked(@JvmField val reason: KagemushaRevocationReason) :
        KagemushaException("KAGEMUSHA device is revoked: $reason")

    class InvalidSignature(detail: String) : KagemushaException("KAGEMUSHA signature is invalid: $detail")

    /** The operation already completed; [existing] is its durable result. */
    class Duplicate(@JvmField val existing: KagemushaOutgoingPayment) :
        KagemushaException("KAGEMUSHA request was already paid")

    class StateLost : KagemushaException("KAGEMUSHA wallet state is lost; recovery is online only")

    class KeyLost : KagemushaException("KAGEMUSHA device key is lost; recovery is online only")

    class IssuerRejected(@JvmField val code: String, detail: String) :
        KagemushaException("KAGEMUSHA issuer rejected the request: $code: $detail")

    class Network(@JvmField val retryable: Boolean, detail: String, cause: Throwable? = null) :
        KagemushaException("KAGEMUSHA issuer is unreachable: $detail", cause)
}

/** Why a receiver refused a payment. Receiver caps and request state are never a reason. */
enum class KagemushaRefusal {
    MALFORMED,
    INVALID_CERT,
    INVALID_SIGNATURE,
    WRONG_RECEIVER,
    REVOKED,
    FORK,
    EXPIRED,
    PAYER_LIMIT,
}

/** Outcome of receiving a payment. Credit is final and immediately spendable offline. */
sealed interface KagemushaReceiveResult {
    /** Credited (or already credited when [duplicate] is true); show [ack] to the payer. */
    class Credited(
        @JvmField val amount: KagemushaAmount,
        paymentId: ByteArray,
        @JvmField val ack: KagemushaPeerMessage,
        @JvmField val duplicate: Boolean,
    ) : KagemushaReceiveResult {
        private val id = paymentId.copyOf()
        fun paymentId(): ByteArray = id.copyOf()
        override fun equals(other: Any?): Boolean = other is Credited && other.amount == amount &&
            other.id.contentEquals(id) && other.ack == ack && other.duplicate == duplicate
        override fun hashCode(): Int = id.contentHashCode()
    }

    /**
     * Refused. When [deliveredLaterIfValid] is true and the payment was valid, the issuer
     * delivers it at this device's next sync after the payer syncs.
     */
    class Refused(
        @JvmField val reason: KagemushaRefusal,
        @JvmField val deliveredLaterIfValid: Boolean,
    ) : KagemushaReceiveResult {
        override fun equals(other: Any?): Boolean = other is Refused && other.reason == reason &&
            other.deliveredLaterIfValid == deliveredLaterIfValid
        override fun hashCode(): Int = reason.hashCode()
        override fun toString(): String = "Refused($reason, deliveredLaterIfValid=$deliveredLaterIfValid)"
    }
}

/** A durable outgoing payment. It is final; [delivered] only records the courtesy Ack. */
class KagemushaOutgoingPayment(
    paymentId: ByteArray,
    @JvmField val amount: KagemushaAmount,
    receiverDeviceId: ByteArray,
    @JvmField val message: KagemushaPeerMessage,
    @JvmField val delivered: Boolean,
    @JvmField val createdAtMs: Long,
) {
    private val id = paymentId.copyOf()
    private val receiver = receiverDeviceId.copyOf()
    fun paymentId(): ByteArray = id.copyOf()
    fun receiverDeviceId(): ByteArray = receiver.copyOf()
    override fun equals(other: Any?): Boolean = other is KagemushaOutgoingPayment && other.id.contentEquals(id) &&
        other.delivered == delivered
    override fun hashCode(): Int = id.contentHashCode()
}

/** A committed load: the reserve transfer and the folded voucher. */
class KagemushaLoadResult(
    @JvmField val amount: KagemushaAmount,
    @JvmField val balance: KagemushaAmount,
    @JvmField val txHash: String,
    voucherId: ByteArray,
) {
    private val voucher = voucherId.copyOf()
    fun voucherId(): ByteArray = voucher.copyOf()
}

/** A committed redemption. It is never cancelled; it can only be frozen on fraud. */
class KagemushaRedeemResult(
    redemptionId: ByteArray,
    @JvmField val amount: KagemushaAmount,
    @JvmField val state: KagemushaRedemptionState,
    /** Ledger transaction hash once [state] is PAID. */
    @JvmField val txHash: String?,
) {
    private val id = redemptionId.copyOf()
    fun redemptionId(): ByteArray = id.copyOf()
}

/** Result of applying one sync receipt. Sync never confirms, holds or reverses a payment. */
class KagemushaSyncResult(
    @JvmField val ackedSeq: Long,
    @JvmField val deliveredCredits: List<KagemushaAmount>,
    @JvmField val redemptions: List<KagemushaRedeemResult>,
    @JvmField val certificateRenewed: Boolean,
    @JvmField val crlEpoch: Long,
    @JvmField val status: KagemushaStatus,
)

/** Account-controller proof for enrollment, signed by the app's existing wallet key. */
class KagemushaAccountProof(
    /** The controller public key in Iroha multihash text form. */
    @JvmField val publicKey: String,
    signature: ByteArray,
) {
    private val sig = signature.copyOf()

    init {
        require(publicKey.isNotEmpty() && publicKey.length <= 512) { "account proof public key is empty or too long" }
        require(sig.isNotEmpty() && sig.size <= 4_096) { "account proof signature is empty or too long" }
    }

    fun signature(): ByteArray = sig.copyOf()
}

/** Ledger access supplied by the app's existing wallet signer. */
interface KagemushaLedgerPort {
    /** The enrolled, single-controller account. Redemptions always pay this account. */
    val accountId: String

    /** Sign [message] with the account controller key. */
    suspend fun signAccountProof(message: ByteArray): KagemushaAccountProof

    /**
     * Submit one `Transfer(asset, amount, account -> reserve)` carrying [metadata], wait for it to
     * commit and return its transaction hash.
     */
    suspend fun transferToReserve(
        asset: String,
        reserve: String,
        amount: KagemushaAmount,
        metadata: Map<String, String>,
    ): String
}

/** Wall-clock source. Time is advisory: nothing locks on clock regression. */
fun interface KagemushaClock {
    fun nowMs(): Long

    companion object {
        @JvmField
        val SYSTEM: KagemushaClock = KagemushaClock { System.currentTimeMillis() }
    }
}

/**
 * Pinned scheme identity for one wallet.
 *
 * A descriptor with `allow_test_devices = true` is refused unless [testing] is set, which apps
 * must only do in debug builds; software test keys are refused otherwise.
 */
class KagemushaAttestedConfig @JvmOverloads constructor(
    schemeId: ByteArray,
    schemeRootPublicKey: ByteArray,
    @JvmField val testing: Boolean = false,
) {
    private val scheme = schemeId.copyOf()
    private val root = KagemushaAttestedCrypto.sec1(KagemushaAttestedCrypto.publicKey(schemeRootPublicKey))

    init {
        require(scheme.size == 32) { "scheme id must be 32 bytes" }
    }

    fun schemeId(): ByteArray = scheme.copyOf()
    fun schemeRootPublicKey(): ByteArray = root.copyOf()
}
