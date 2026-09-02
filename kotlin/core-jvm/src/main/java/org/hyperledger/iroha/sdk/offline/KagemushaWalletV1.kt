// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.util.Collections
import java.util.EnumSet
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock

/** Hardware properties required together by every production Kagemusha V1 provider. */
enum class KagemushaHardwareCapabilityV1 {
    EXACT_NEXT_PREDECESSOR_CONSUMPTION,
    ONE_USE_SUCCESSOR_AUTHORIZATION,
    ROLLBACK_RESISTANT_COUNTER_AND_JOURNAL,
    SEALED_TRANSITION_RECOVERY,
    RECEIVER_BOUND_CREDIT_COMMIT,
    ROLLBACK_RESISTANT_ACCEPTED_CREDIT_INBOX,
    AUTHENTICATED_INBOUND_STAGING,
    AUTHORITATIVE_REPLAY_ROOT_RECOVERY,
    SENDER_OUTBOX_RESERVATION,
    AUTHENTICATED_DURABLE_RETRY_OUTBOX,
    ATOMIC_VERIFIED_CANDIDATE_COMMIT,
    RECOVERABLE_TERMINAL_COMMIT_CERTIFICATE,
    TRUSTED_TIME_OR_LEASE,
    KAGEMUSHA_HARDWARE_EPOCH_ROTATION,
    ROLLBACK_SAFE_COUNTER_ROLLOVER,
    NO_SOFTWARE_FALLBACK,
}

/** Authenticated release and hardware credential returned by the shared native core. */
class KagemushaHardwareQualificationV1(
    @JvmField val protocolVersion: Int,
    @JvmField val profile: KagemushaHardwareProfileV1,
    @JvmField val credential: KagemushaHardwareCredentialV1,
    releaseId: ByteArray,
    capabilities: Set<KagemushaHardwareCapabilityV1>,
) {
    private val releaseIdValue = fixed32(releaseId, "releaseId")
    private val capabilityValues = if (capabilities.isEmpty()) {
        EnumSet.noneOf(KagemushaHardwareCapabilityV1::class.java)
    } else {
        EnumSet.copyOf(capabilities)
    }

    /** Return the authenticated release identity selected by native core. */
    fun releaseId(): ByteArray = releaseIdValue.copyOf()

    /** Return a defensive immutable copy of the attested capability set. */
    fun capabilities(): Set<KagemushaHardwareCapabilityV1> =
        Collections.unmodifiableSet(EnumSet.copyOf(capabilityValues))

    /** Reject a partial, old, expired, or software-backed provider. */
    fun requireProductionReady() {
        require(protocolVersion == KagemushaWireV1.WIRE_VERSION)
        require(profile.version == protocolVersion && profile.protocolVersion == protocolVersion)
        require(credential.version == protocolVersion)
        require(profile.hardwareProfileId().contentEquals(credential.hardwareProfileId()))
        require(profile.policyEpoch == credential.policyEpoch)
        require(capabilityValues == EnumSet.allOf(KagemushaHardwareCapabilityV1::class.java)) {
            "Kagemusha V1 requires the complete non-forking hardware capability set"
        }
    }
}

/** Recovery result after native core has resolved every interrupted prepare/commit operation. */
class KagemushaHardwareRecoveryV1(
    aggregateState: ByteArray?,
    @JvmField val journalRevision: BigInteger,
    @JvmField val pendingCreditCount: BigInteger,
    @JvmField val retryOutboxCount: BigInteger,
) {
    private val aggregateStateValue = aggregateState?.copyOf()

    init {
        requireUnsigned128(journalRevision, "journalRevision")
        require(pendingCreditCount.signum() >= 0)
        require(retryOutboxCount.signum() >= 0)
    }

    /** Canonical aggregate state, or `null` before bootstrap. */
    fun aggregateState(): ByteArray? = aggregateStateValue?.copyOf()
}

/** Whether inbound data was newly staged or matched an exact durable duplicate. */
enum class KagemushaHardwareStageDispositionV1 { STAGED, EXACT_DUPLICATE }

/** Durable result of staging one peer payment. */
class KagemushaHardwarePaymentStageV1(
    @JvmField val disposition: KagemushaHardwareStageDispositionV1,
    creditId: ByteArray,
    acknowledgement: ByteArray,
) {
    private val creditIdValue = fixed32(creditId, "creditId")
    private val acknowledgementValue = acknowledgement.copyOf()

    init {
        require(acknowledgementValue.isNotEmpty())
        require(acknowledgementValue.size <= KagemushaWireV1.MAXIMUM_ACKNOWLEDGEMENT_BYTES)
    }

    fun creditId(): ByteArray = creditIdValue.copyOf()
    fun acknowledgement(): ByteArray = acknowledgementValue.copyOf()
}

/** Durable result of staging one finalized mint credit. */
class KagemushaHardwareMintStageV1(
    @JvmField val disposition: KagemushaHardwareStageDispositionV1,
    creditId: ByteArray,
) {
    private val creditIdValue = fixed32(creditId, "creditId")
    fun creditId(): ByteArray = creditIdValue.copyOf()
}

/** Native terminal envelope plus the authoritative private successor's public commitment. */
class KagemushaHardwareTerminalResultV1(
    canonicalEnvelope: ByteArray,
    aggregateState: ByteArray,
) {
    private val canonicalEnvelopeValue = canonicalEnvelope.copyOf()
    private val aggregateStateValue = aggregateState.copyOf()

    init {
        require(canonicalEnvelopeValue.isNotEmpty())
        require(aggregateStateValue.isNotEmpty())
    }

    fun canonicalEnvelope(): ByteArray = canonicalEnvelopeValue.copyOf()
    fun aggregateState(): ByteArray = aggregateStateValue.copyOf()
}

/** Result of structurally decoding a durable acknowledgement from native staging. */
class KagemushaStagedPaymentV1(
    @JvmField val disposition: KagemushaHardwareStageDispositionV1,
    @JvmField val acknowledgement: KagemushaAcknowledgementV1,
    canonicalAcknowledgement: ByteArray,
) {
    private val canonicalAcknowledgementValue = canonicalAcknowledgement.copyOf()
    fun canonicalAcknowledgement(): ByteArray = canonicalAcknowledgementValue.copyOf()
}

/**
 * Mandatory shared-native-core and non-forking secure-device boundary.
 *
 * Implementations authenticate releases, profiles, credentials, signatures, recursive proofs,
 * X25519 possession, HKDF/AEAD openings, exact amount binding, inbox reservations, and
 * recoverable prepare/prove/commit state. No method may fall back to process memory, application
 * files, ordinary AndroidKeyStore signing, or JVM cryptography for monetary authority.
 */
interface KagemushaHardwareProviderV1 {
    /** Return the currently authenticated release/profile/credential tuple. */
    fun qualification(): KagemushaHardwareQualificationV1

    /** Resolve interrupted transitions and recover authoritative durable state. */
    fun recover(): KagemushaHardwareRecoveryV1

    /** Establish the hardware-bound zero state, idempotently. */
    fun bootstrapState(): ByteArray

    /** Create and sign one exact-amount request using hardware trusted time. */
    fun createPaymentRequest(
        recipientAccount: ByteArray,
        amount: BigInteger,
        validityWindowMillis: Long,
    ): ByteArray

    /** Stage one verified peer payment and durably sign its acknowledgement. */
    fun stagePayment(
        canonicalRequest: ByteArray,
        canonicalPayment: ByteArray,
    ): KagemushaHardwarePaymentStageV1

    /** Stage one mint credit only after verifying its exact pre-debit authorization. */
    fun stageMintCredit(
        canonicalAuthorization: ByteArray,
        canonicalMintCredit: ByteArray,
    ): KagemushaHardwareMintStageV1

    /** Snapshot the current durable inbox high-water mark without a count maximum. */
    fun pendingCreditWatermark(): BigInteger

    /** Return the rollback-resistant journal revision. */
    fun journalRevision(): BigInteger

    /** Fold one staged credit from a stable inbox snapshot, or return null when drained. */
    fun foldPendingCredit(inboxSequenceInclusive: BigInteger): ByteArray?

    /** Prepare, prove, commit, and install one payment; return byte-identical terminal output. */
    fun commitPayment(canonicalRequest: ByteArray): KagemushaHardwareTerminalResultV1

    /** Recover one byte-identical payment from the authenticated durable retry outbox. */
    fun recoverPayment(creditId: ByteArray): ByteArray?

    /** Verify and record an acknowledgement before releasing the matching outbox entry. */
    fun recordAcknowledgement(creditId: ByteArray, canonicalAcknowledgement: ByteArray)

    /** Prepare, prove, commit, and install one terminal redemption. */
    fun commitRedemption(amount: BigInteger, beneficiaryAccount: ByteArray): KagemushaHardwareTerminalResultV1

    /** Recover one byte-identical redemption voucher. */
    fun recoverRedemption(redemptionId: ByteArray): ByteArray?

    /** Rotate the complete private aggregate state and replay root to a qualified epoch. */
    fun rotateHardwareEpoch(): ByteArray
}

/** Aggregate-balance Kagemusha V1 orchestration over the authoritative native boundary. */
class KagemushaWalletV1 private constructor(
    private val provider: KagemushaHardwareProviderV1,
    initialQualification: KagemushaHardwareQualificationV1,
    initialState: KagemushaAggregateStateCommitmentV1,
    initialJournalRevision: BigInteger,
) {
    private val transitionLock = ReentrantLock()

    @Volatile
    private var currentQualification = initialQualification

    @Volatile
    private var currentAggregateState = initialState

    @Volatile
    private var currentJournalRevision = initialJournalRevision

    /** Return the current authenticated compact hardware credential. */
    fun hardwareCredential(): KagemushaHardwareCredentialV1 = currentQualification.credential

    /** Return the latest native-authoritative aggregate-state commitment. */
    fun aggregateState(): KagemushaAggregateStateCommitmentV1 = currentAggregateState

    /** Return the latest rollback-resistant journal revision. */
    fun journalRevision(): BigInteger = currentJournalRevision

    /** Recover interrupted work and refresh authoritative state. */
    fun recover(): KagemushaHardwareRecoveryV1 = transitionLock.withLock {
        val qualification = requireQualified(provider.qualification())
        val recovery = provider.recover()
        val stateBytes = recovery.aggregateState() ?: provider.bootstrapState()
        val state = KagemushaNoritoV1.decodeAggregateStateShapeExact(stateBytes)
        requireStateQualification(state, qualification)
        val revision = provider.journalRevision()
        require(revision == recovery.journalRevision)
        currentQualification = qualification
        currentAggregateState = state
        currentJournalRevision = revision
        KagemushaHardwareRecoveryV1(
            KagemushaNoritoV1.encodeAggregateStateShape(state),
            revision,
            recovery.pendingCreditCount,
            recovery.retryOutboxCount,
        )
    }

    /** Create one exact-amount request reusable by any number of valid payments. */
    fun createPaymentRequest(
        recipient: KagemushaAccountIdV1,
        amount: BigInteger,
        validityWindowMillis: Long,
    ): KagemushaPaymentRequestV1 = transitionLock.withLock {
        requirePositiveU128(amount, "amount")
        require(validityWindowMillis in 1..KagemushaWireV1.REQUEST_MAX_TTL_MS)
        val request = KagemushaNoritoV1.decodePaymentRequestShapeExact(
            provider.createPaymentRequest(recipient.canonicalPayload(), amount, validityWindowMillis),
        )
        require(request.recipient == recipient)
        require(request.amount == amount)
        require(request.expiresAtMs - request.issuedAtMs == validityWindowMillis)
        requireStateRequestBinding(currentAggregateState, request)
        request
    }

    /** Prepare, prove, atomically commit, and return a receiver-bound payment. */
    fun send(
        request: KagemushaPaymentRequestV1,
    ): KagemushaPaymentV1 = transitionLock.withLock {
        drainPendingCreditsLocked()
        val canonicalRequest = KagemushaNoritoV1.encodePaymentRequestShape(request)
        val result = provider.commitPayment(canonicalRequest)
        val payment = KagemushaNoritoV1.decodePaymentShapeExact(result.canonicalEnvelope(), request)
        installAuthoritativeState(result.aggregateState())
        payment
    }

    /** Stage a payment and return its durable acknowledgement. */
    fun stagePayment(
        request: KagemushaPaymentRequestV1,
        payment: KagemushaPaymentV1,
    ): KagemushaStagedPaymentV1 = transitionLock.withLock {
        val canonicalRequest = KagemushaNoritoV1.encodePaymentRequestShape(request)
        val canonicalPayment = KagemushaNoritoV1.encodePaymentShape(payment, request)
        val before = currentJournalRevision
        val staged = provider.stagePayment(canonicalRequest, canonicalPayment)
        require(staged.creditId().contentEquals(payment.statement.lifecycle.creditId()))
        val canonicalAcknowledgement = staged.acknowledgement()
        val acknowledgement = KagemushaNoritoV1.decodeAcknowledgementShapeExact(
            canonicalAcknowledgement,
            request,
            payment,
        )
        val after = provider.journalRevision()
        when (staged.disposition) {
            KagemushaHardwareStageDispositionV1.STAGED -> require(after == before + BigInteger.ONE)
            KagemushaHardwareStageDispositionV1.EXACT_DUPLICATE -> require(after == before)
        }
        currentJournalRevision = after
        KagemushaStagedPaymentV1(staged.disposition, acknowledgement, canonicalAcknowledgement)
    }

    /** Stage a finalized reserve-backed mint only with its exact pre-debit authorization. */
    fun stageMintCredit(
        authorization: KagemushaMintAuthorizationV1,
        mintCredit: KagemushaMintCreditV1,
    ): KagemushaHardwareStageDispositionV1 = transitionLock.withLock {
        val canonicalAuthorization = KagemushaNoritoV1.encodeMintAuthorizationShape(authorization)
        val canonicalCredit = KagemushaNoritoV1.encodeMintCreditShape(mintCredit, authorization)
        val before = currentJournalRevision
        val staged = provider.stageMintCredit(canonicalAuthorization, canonicalCredit)
        require(staged.creditId().contentEquals(mintCredit.statement.lifecycle.creditId()))
        val after = provider.journalRevision()
        when (staged.disposition) {
            KagemushaHardwareStageDispositionV1.STAGED -> require(after == before + BigInteger.ONE)
            KagemushaHardwareStageDispositionV1.EXACT_DUPLICATE -> require(after == before)
        }
        currentJournalRevision = after
        staged.disposition
    }

    /** Fold one staged credit from the current snapshot. */
    fun foldPendingCredit(): Boolean = transitionLock.withLock { foldSnapshotLocked() }

    /** Drain one stable inbox snapshot through repeated one-credit transitions. */
    fun drainPendingCredits(): BigInteger = transitionLock.withLock { drainPendingCreditsLocked() }

    /** Recover a byte-identical exposed payment for transport retry. */
    fun recoverPayment(
        request: KagemushaPaymentRequestV1,
        creditId: ByteArray,
    ): KagemushaPaymentV1? {
        val expected = fixed32(creditId, "creditId")
        val canonical = provider.recoverPayment(expected) ?: return null
        val payment = KagemushaNoritoV1.decodePaymentShapeExact(canonical, request)
        require(payment.statement.lifecycle.creditId().contentEquals(expected))
        return payment
    }

    /** Record a shape-valid ACK; native core authenticates it before outbox release. */
    fun recordAcknowledgement(
        request: KagemushaPaymentRequestV1,
        payment: KagemushaPaymentV1,
        acknowledgement: KagemushaAcknowledgementV1,
    ) {
        val canonical = KagemushaNoritoV1.encodeAcknowledgementShape(acknowledgement, request, payment)
        provider.recordAcknowledgement(payment.statement.lifecycle.creditId(), canonical)
    }

    /** Prepare, prove, commit, and install one full or partial terminal redemption. */
    fun redeem(
        amount: BigInteger,
        beneficiary: KagemushaAccountIdV1,
    ): KagemushaRedemptionVoucherV1 = transitionLock.withLock {
        requirePositiveU128(amount, "amount")
        drainPendingCreditsLocked()
        val result = provider.commitRedemption(amount, beneficiary.canonicalPayload())
        val voucher = KagemushaNoritoV1.decodeRedemptionVoucherShapeExact(result.canonicalEnvelope())
        require(voucher.statement.amount == amount && voucher.statement.beneficiary == beneficiary)
        installAuthoritativeState(result.aggregateState())
        voucher
    }

    /** Recover one byte-identical terminal redemption voucher. */
    fun recoverRedemption(redemptionId: ByteArray): KagemushaRedemptionVoucherV1? {
        val expected = fixed32(redemptionId, "redemptionId")
        val canonical = provider.recoverRedemption(expected) ?: return null
        val voucher = KagemushaNoritoV1.decodeRedemptionVoucherShapeExact(canonical)
        require(voucher.statement.redemptionId().contentEquals(expected))
        return voucher
    }

    /** Rotate the complete private balance and replay root in qualified hardware. */
    fun rotateHardwareEpoch(): KagemushaAggregateStateCommitmentV1 = transitionLock.withLock {
        drainPendingCreditsLocked()
        installAuthoritativeState(provider.rotateHardwareEpoch())
        currentQualification = requireQualified(provider.qualification())
        requireStateQualification(currentAggregateState, currentQualification)
        currentAggregateState
    }

    private fun foldSnapshotLocked(): Boolean {
        val watermark = provider.pendingCreditWatermark()
        return foldAtWatermarkLocked(watermark)
    }

    private fun drainPendingCreditsLocked(): BigInteger {
        val watermark = provider.pendingCreditWatermark()
        require(watermark.signum() >= 0)
        var total = BigInteger.ZERO
        while (true) {
            if (!foldAtWatermarkLocked(watermark)) return total
            total += BigInteger.ONE
        }
    }

    private fun foldAtWatermarkLocked(watermark: BigInteger): Boolean {
        require(watermark.signum() >= 0)
        val before = currentJournalRevision
        val beforeCommitment = currentAggregateState.stateCommitment()
        val successorBytes = provider.foldPendingCredit(watermark)
        if (successorBytes != null) {
            installAuthoritativeState(successorBytes)
            require(!currentAggregateState.stateCommitment().contentEquals(beforeCommitment)) {
                "receive fold made no aggregate-state progress"
            }
        }
        val after = provider.journalRevision()
        val expectedRevision = if (successorBytes == null) before else before + BigInteger.ONE
        require(after == expectedRevision) {
            "receive fold did not consume exactly one journal revision"
        }
        currentJournalRevision = after
        return successorBytes != null
    }

    private fun installAuthoritativeState(bytes: ByteArray) {
        val state = KagemushaNoritoV1.decodeAggregateStateShapeExact(bytes)
        requireSameAsset(currentAggregateState, state)
        currentAggregateState = state
        currentJournalRevision = provider.journalRevision()
    }

    companion object {
        /** Open only after the complete native/hardware contract and recovery succeed. */
        @JvmStatic
        fun open(provider: KagemushaHardwareProviderV1): KagemushaWalletV1 {
            val qualification = requireQualified(provider.qualification())
            val recovery = provider.recover()
            val stateBytes = recovery.aggregateState() ?: provider.bootstrapState()
            val state = KagemushaNoritoV1.decodeAggregateStateShapeExact(stateBytes)
            requireStateQualification(state, qualification)
            require(provider.journalRevision() == recovery.journalRevision)
            return KagemushaWalletV1(provider, qualification, state, recovery.journalRevision)
        }

        private fun requireQualified(
            qualification: KagemushaHardwareQualificationV1,
        ): KagemushaHardwareQualificationV1 {
            qualification.requireProductionReady()
            return qualification
        }

        private fun requireStateQualification(
            state: KagemushaAggregateStateCommitmentV1,
            qualification: KagemushaHardwareQualificationV1,
        ) {
            require(state.releaseId().contentEquals(qualification.releaseId()))
            require(state.networkId == qualification.credential.networkId)
            require(state.keyReference().contentEquals(qualification.credential.deviceKeyReference()))
            require(state.hardwarePolicyId().contentEquals(qualification.profile.hardwareProfileId()))
        }

        private fun requireStateRequestBinding(
            state: KagemushaAggregateStateCommitmentV1,
            request: KagemushaPaymentRequestV1,
        ) {
            require(request.releaseId().contentEquals(state.releaseId()))
            require(request.networkId == state.networkId)
            require(request.asset == state.asset && request.assetIncarnation == state.assetIncarnation)
            require(request.scale == state.scale)
            require(request.liabilityPoolId().contentEquals(state.liabilityPoolId()))
        }

        private fun requireSameAsset(
            before: KagemushaAggregateStateCommitmentV1,
            after: KagemushaAggregateStateCommitmentV1,
        ) {
            require(after.version == before.version)
            require(after.releaseId().contentEquals(before.releaseId()))
            require(after.networkId == before.networkId)
            require(after.asset == before.asset && after.assetIncarnation == before.assetIncarnation)
            require(after.scale == before.scale)
            require(after.liabilityPoolId().contentEquals(before.liabilityPoolId()))
            require(after.laneId().contentEquals(before.laneId()))
        }

    }
}
