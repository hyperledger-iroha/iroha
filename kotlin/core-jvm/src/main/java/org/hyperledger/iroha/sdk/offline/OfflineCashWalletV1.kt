// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.util.Collections
import java.util.EnumSet
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock

/** Hardware properties required together by every production Offline Cash V1 provider. */
enum class OfflineCashHardwareCapabilityV1 {
    EXACT_NEXT_PREDECESSOR_CONSUMPTION,
    ONE_USE_SUCCESSOR_AUTHORIZATION,
    ROLLBACK_RESISTANT_COUNTER_AND_JOURNAL,
    SEALED_TRANSITION_RECOVERY,
    ONE_USE_ACCEPTANCE_TICKETS,
    DURABLE_INBOX_RESERVATION,
    AUTHENTICATED_INBOUND_STAGING,
    AUTHORITATIVE_REPLAY_ROOT_RECOVERY,
    SENDER_OUTBOX_RESERVATION,
    AUTHENTICATED_DURABLE_RETRY_OUTBOX,
    ATOMIC_VERIFIED_CANDIDATE_COMMIT,
    RECOVERABLE_TERMINAL_COMMIT_CERTIFICATE,
    TRUSTED_TIME_OR_LEASE,
    OFFLINE_HARDWARE_EPOCH_ROTATION,
    ROLLBACK_SAFE_COUNTER_ROLLOVER,
    NO_SOFTWARE_FALLBACK,
}

/** Authenticated release and hardware credential returned by the shared native core. */
class OfflineCashHardwareQualificationV1(
    @JvmField val protocolVersion: Int,
    @JvmField val profile: OfflineCashHardwareProfileV1,
    @JvmField val credential: OfflineCashHardwareCredentialV1,
    releaseId: ByteArray,
    capabilities: Set<OfflineCashHardwareCapabilityV1>,
) {
    private val releaseIdValue = fixed32(releaseId, "releaseId")
    private val capabilityValues = if (capabilities.isEmpty()) {
        EnumSet.noneOf(OfflineCashHardwareCapabilityV1::class.java)
    } else {
        EnumSet.copyOf(capabilities)
    }

    /** Return the authenticated release identity selected by native core. */
    fun releaseId(): ByteArray = releaseIdValue.copyOf()

    /** Return a defensive immutable copy of the attested capability set. */
    fun capabilities(): Set<OfflineCashHardwareCapabilityV1> =
        Collections.unmodifiableSet(EnumSet.copyOf(capabilityValues))

    /** Reject a partial, old, expired, or software-backed provider. */
    fun requireProductionReady() {
        require(protocolVersion == OfflineCashWireV1.WIRE_VERSION)
        require(profile.version == protocolVersion && profile.protocolVersion == protocolVersion)
        require(credential.version == protocolVersion)
        require(profile.hardwareProfileId().contentEquals(credential.hardwareProfileId()))
        require(profile.policyEpoch == credential.policyEpoch)
        require(capabilityValues == EnumSet.allOf(OfflineCashHardwareCapabilityV1::class.java)) {
            "Offline Cash V1 requires the complete non-forking hardware capability set"
        }
    }
}

/** Recovery result after native core has resolved every interrupted prepare/commit operation. */
class OfflineCashHardwareRecoveryV1(
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
enum class OfflineCashHardwareStageDispositionV1 { STAGED, EXACT_DUPLICATE }

/** Durable result of staging one peer payment. */
class OfflineCashHardwarePaymentStageV1(
    @JvmField val disposition: OfflineCashHardwareStageDispositionV1,
    creditId: ByteArray,
    acknowledgement: ByteArray,
) {
    private val creditIdValue = fixed32(creditId, "creditId")
    private val acknowledgementValue = acknowledgement.copyOf()

    init {
        require(acknowledgementValue.isNotEmpty())
        require(acknowledgementValue.size <= OfflineCashWireV1.MAXIMUM_ACKNOWLEDGEMENT_BYTES)
    }

    fun creditId(): ByteArray = creditIdValue.copyOf()
    fun acknowledgement(): ByteArray = acknowledgementValue.copyOf()
}

/** Durable result of staging one finalized mint credit. */
class OfflineCashHardwareMintStageV1(
    @JvmField val disposition: OfflineCashHardwareStageDispositionV1,
    creditId: ByteArray,
) {
    private val creditIdValue = fixed32(creditId, "creditId")
    fun creditId(): ByteArray = creditIdValue.copyOf()
}

/** Native-core result for a fixed-shape batch fold of one through sixteen credits. */
class OfflineCashHardwareFoldBatchV1(
    @JvmField val foldedCredits: Int,
    aggregateState: ByteArray?,
) {
    private val aggregateStateValue = aggregateState?.copyOf()

    init {
        require(foldedCredits in 0..16)
        require((foldedCredits == 0) == (aggregateStateValue == null))
    }

    /** Canonical successor state when at least one credit was folded. */
    fun aggregateState(): ByteArray? = aggregateStateValue?.copyOf()
}

/** Native terminal envelope plus the authoritative private successor's public commitment. */
class OfflineCashHardwareTerminalResultV1(
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
class OfflineCashStagedPaymentV1(
    @JvmField val disposition: OfflineCashHardwareStageDispositionV1,
    @JvmField val acknowledgement: OfflineCashAcknowledgementV1,
    canonicalAcknowledgement: ByteArray,
) {
    private val canonicalAcknowledgementValue = canonicalAcknowledgement.copyOf()
    fun canonicalAcknowledgement(): ByteArray = canonicalAcknowledgementValue.copyOf()
}

/**
 * Mandatory shared-native-core and non-forking secure-device boundary.
 *
 * Implementations authenticate releases, profiles, credentials, signatures, recursive proofs,
 * X25519 possession, HKDF/AEAD openings, exact request-ledger decisions, inbox reservations, and
 * recoverable prepare/prove/commit state. No method may fall back to process memory, application
 * files, ordinary AndroidKeyStore signing, or JVM cryptography for monetary authority.
 */
interface OfflineCashHardwareProviderV1 {
    /** Return the currently authenticated release/profile/credential tuple. */
    fun qualification(): OfflineCashHardwareQualificationV1

    /** Resolve interrupted transitions and recover authoritative durable state. */
    fun recover(): OfflineCashHardwareRecoveryV1

    /** Establish the hardware-bound zero state, idempotently. */
    fun bootstrapState(): ByteArray

    /** Create and sign one reusable request policy using hardware trusted time. */
    fun createPaymentRequest(
        recipientAccount: ByteArray,
        requestMode: ByteArray,
        validityWindowMillis: Long,
    ): ByteArray

    /** Prove sender capability before the receiver persists intent or consumes capacity. */
    fun createAcceptanceIntentAuthorization(
        canonicalRequest: ByteArray,
        exactAmount: BigInteger,
    ): ByteArray

    /** Verify sender authorization, atomically decide intent, and issue one reserved ticket. */
    fun issueAcceptanceTicket(
        canonicalRequest: ByteArray,
        canonicalAuthorization: ByteArray,
    ): ByteArray

    /** Stage one verified peer payment and durably sign its acknowledgement. */
    fun stagePayment(
        canonicalRequest: ByteArray,
        canonicalPayment: ByteArray,
    ): OfflineCashHardwarePaymentStageV1

    /** Stage one mint credit only after verifying its exact pre-debit authorization. */
    fun stageMintCredit(
        canonicalAuthorization: ByteArray,
        canonicalMintCredit: ByteArray,
    ): OfflineCashHardwareMintStageV1

    /** Snapshot the current durable inbox high-water mark without a count maximum. */
    fun pendingCreditWatermark(): BigInteger

    /** Return the rollback-resistant journal revision. */
    fun journalRevision(): BigInteger

    /** Fold a padded fixed-shape batch of at most sixteen credits from one stable snapshot. */
    fun foldPendingCreditBatch(inboxSequenceInclusive: BigInteger, maximumCredits: Int): OfflineCashHardwareFoldBatchV1

    /** Prepare, prove, commit, and install one payment; return byte-identical terminal output. */
    fun commitPayment(
        canonicalRequest: ByteArray,
        canonicalAuthorization: ByteArray,
        canonicalTicket: ByteArray,
    ): OfflineCashHardwareTerminalResultV1

    /** Recover one byte-identical payment from the authenticated durable retry outbox. */
    fun recoverPayment(creditId: ByteArray): ByteArray?

    /** Verify and record an acknowledgement before releasing the matching outbox entry. */
    fun recordAcknowledgement(creditId: ByteArray, canonicalAcknowledgement: ByteArray)

    /** Prepare, prove, commit, and install one terminal redemption. */
    fun commitRedemption(amount: BigInteger, beneficiaryAccount: ByteArray): OfflineCashHardwareTerminalResultV1

    /** Recover one byte-identical redemption voucher. */
    fun recoverRedemption(redemptionId: ByteArray): ByteArray?

    /** Rotate the complete private aggregate state and replay root to a qualified epoch. */
    fun rotateHardwareEpoch(): ByteArray
}

/** Aggregate-balance Offline Cash V1 orchestration over the authoritative native boundary. */
class OfflineCashWalletV1 private constructor(
    private val provider: OfflineCashHardwareProviderV1,
    initialQualification: OfflineCashHardwareQualificationV1,
    initialState: OfflineCashAggregateStateCommitmentV1,
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
    fun hardwareCredential(): OfflineCashHardwareCredentialV1 = currentQualification.credential

    /** Return the latest native-authoritative aggregate-state commitment. */
    fun aggregateState(): OfflineCashAggregateStateCommitmentV1 = currentAggregateState

    /** Return the latest rollback-resistant journal revision. */
    fun journalRevision(): BigInteger = currentJournalRevision

    /** Recover interrupted work and refresh authoritative state. */
    fun recover(): OfflineCashHardwareRecoveryV1 = transitionLock.withLock {
        val qualification = requireQualified(provider.qualification())
        val recovery = provider.recover()
        val stateBytes = recovery.aggregateState() ?: provider.bootstrapState()
        val state = OfflineCashNoritoV1.decodeAggregateStateShapeExact(stateBytes)
        requireStateQualification(state, qualification)
        val revision = provider.journalRevision()
        require(revision == recovery.journalRevision)
        currentQualification = qualification
        currentAggregateState = state
        currentJournalRevision = revision
        OfflineCashHardwareRecoveryV1(
            OfflineCashNoritoV1.encodeAggregateStateShape(state),
            revision,
            recovery.pendingCreditCount,
            recovery.retryOutboxCount,
        )
    }

    /** Create one reusable request; each actual payment still requires a one-use ticket. */
    fun createPaymentRequest(
        recipient: OfflineCashAccountIdV1,
        requestMode: OfflineCashPaymentRequestModeV1,
        validityWindowMillis: Long,
    ): OfflineCashPaymentRequestV1 = transitionLock.withLock {
        require(validityWindowMillis in 1..OfflineCashWireV1.REQUEST_MAX_TTL_MS)
        val canonicalMode = OfflineCashNoritoV1.encodePaymentRequestModeShape(requestMode)
        val request = OfflineCashNoritoV1.decodePaymentRequestShapeExact(
            provider.createPaymentRequest(recipient.canonicalPayload(), canonicalMode, validityWindowMillis),
        )
        require(request.recipient == recipient)
        require(request.expiresAtMs - request.issuedAtMs == validityWindowMillis)
        requireStateRequestBinding(currentAggregateState, request)
        request
    }

    /** Ask sender hardware for the proof that must precede any receiver-side persistence. */
    fun authorizeAcceptanceIntent(
        request: OfflineCashPaymentRequestV1,
        exactAmount: BigInteger,
    ): OfflineCashAcceptanceIntentAuthorizationV1 = transitionLock.withLock {
        require(request.requestMode.acceptsExactAmount(exactAmount))
        val canonicalRequest = OfflineCashNoritoV1.encodePaymentRequestShape(request)
        OfflineCashNoritoV1.decodeAcceptanceIntentAuthorizationShapeExact(
            provider.createAcceptanceIntentAuthorization(canonicalRequest, exactAmount),
            request,
        )
    }

    /** Verify sender proof in native core, then atomically reserve and issue one ticket. */
    fun issueAcceptanceTicket(
        request: OfflineCashPaymentRequestV1,
        authorization: OfflineCashAcceptanceIntentAuthorizationV1,
    ): OfflineCashAcceptanceTicketV1 = transitionLock.withLock {
        val canonicalRequest = OfflineCashNoritoV1.encodePaymentRequestShape(request)
        val canonicalAuthorization =
            OfflineCashNoritoV1.encodeAcceptanceIntentAuthorizationShape(authorization, request)
        OfflineCashNoritoV1.decodeAcceptanceTicketShapeExact(
            provider.issueAcceptanceTicket(canonicalRequest, canonicalAuthorization),
            request,
            authorization,
        )
    }

    /** Prepare, prove, atomically commit, and return a receiver-bound payment. */
    fun send(
        request: OfflineCashPaymentRequestV1,
        authorization: OfflineCashAcceptanceIntentAuthorizationV1,
        ticket: OfflineCashAcceptanceTicketV1,
    ): OfflineCashPaymentV1 = transitionLock.withLock {
        drainPendingCreditsLocked()
        val canonicalRequest = OfflineCashNoritoV1.encodePaymentRequestShape(request)
        val canonicalAuthorization =
            OfflineCashNoritoV1.encodeAcceptanceIntentAuthorizationShape(authorization, request)
        val canonicalTicket = OfflineCashNoritoV1.encodeAcceptanceTicketShape(ticket, request, authorization)
        val result = provider.commitPayment(canonicalRequest, canonicalAuthorization, canonicalTicket)
        val payment = OfflineCashNoritoV1.decodePaymentShapeExact(result.canonicalEnvelope(), request)
        require(payment.acceptanceTicket.acceptanceTicketId().contentEquals(ticket.acceptanceTicketId()))
        installAuthoritativeState(result.aggregateState())
        payment
    }

    /** Stage a payment and return its durable acknowledgement. */
    fun stagePayment(
        request: OfflineCashPaymentRequestV1,
        payment: OfflineCashPaymentV1,
    ): OfflineCashStagedPaymentV1 = transitionLock.withLock {
        val canonicalRequest = OfflineCashNoritoV1.encodePaymentRequestShape(request)
        val canonicalPayment = OfflineCashNoritoV1.encodePaymentShape(payment, request)
        val before = currentJournalRevision
        val staged = provider.stagePayment(canonicalRequest, canonicalPayment)
        require(staged.creditId().contentEquals(payment.statement.lifecycle.creditId()))
        val canonicalAcknowledgement = staged.acknowledgement()
        val acknowledgement = OfflineCashNoritoV1.decodeAcknowledgementShapeExact(
            canonicalAcknowledgement,
            request,
            payment,
        )
        val after = provider.journalRevision()
        when (staged.disposition) {
            OfflineCashHardwareStageDispositionV1.STAGED -> require(after == before + BigInteger.ONE)
            OfflineCashHardwareStageDispositionV1.EXACT_DUPLICATE -> require(after == before)
        }
        currentJournalRevision = after
        OfflineCashStagedPaymentV1(staged.disposition, acknowledgement, canonicalAcknowledgement)
    }

    /** Stage a finalized reserve-backed mint only with its exact pre-debit authorization. */
    fun stageMintCredit(
        authorization: OfflineCashMintAuthorizationV1,
        mintCredit: OfflineCashMintCreditV1,
    ): OfflineCashHardwareStageDispositionV1 = transitionLock.withLock {
        val canonicalAuthorization = OfflineCashNoritoV1.encodeMintAuthorizationShape(authorization)
        val canonicalCredit = OfflineCashNoritoV1.encodeMintCreditShape(mintCredit, authorization)
        val before = currentJournalRevision
        val staged = provider.stageMintCredit(canonicalAuthorization, canonicalCredit)
        require(staged.creditId().contentEquals(mintCredit.statement.lifecycle.creditId()))
        val after = provider.journalRevision()
        when (staged.disposition) {
            OfflineCashHardwareStageDispositionV1.STAGED -> require(after == before + BigInteger.ONE)
            OfflineCashHardwareStageDispositionV1.EXACT_DUPLICATE -> require(after == before)
        }
        currentJournalRevision = after
        staged.disposition
    }

    /** Fold one fixed-shape batch of up to sixteen credits from the current snapshot. */
    fun foldPendingCreditBatch(): Int = transitionLock.withLock { foldSnapshotLocked() }

    /** Drain one stable inbox snapshot through repeated fixed one-to-sixteen-credit batches. */
    fun drainPendingCredits(): BigInteger = transitionLock.withLock { drainPendingCreditsLocked() }

    /** Recover a byte-identical exposed payment for transport retry. */
    fun recoverPayment(
        request: OfflineCashPaymentRequestV1,
        creditId: ByteArray,
    ): OfflineCashPaymentV1? {
        val expected = fixed32(creditId, "creditId")
        val canonical = provider.recoverPayment(expected) ?: return null
        val payment = OfflineCashNoritoV1.decodePaymentShapeExact(canonical, request)
        require(payment.statement.lifecycle.creditId().contentEquals(expected))
        return payment
    }

    /** Record a shape-valid ACK; native core authenticates it before outbox release. */
    fun recordAcknowledgement(
        request: OfflineCashPaymentRequestV1,
        payment: OfflineCashPaymentV1,
        acknowledgement: OfflineCashAcknowledgementV1,
    ) {
        val canonical = OfflineCashNoritoV1.encodeAcknowledgementShape(acknowledgement, request, payment)
        provider.recordAcknowledgement(payment.statement.lifecycle.creditId(), canonical)
    }

    /** Prepare, prove, commit, and install one full or partial terminal redemption. */
    fun redeem(
        amount: BigInteger,
        beneficiary: OfflineCashAccountIdV1,
    ): OfflineCashRedemptionVoucherV1 = transitionLock.withLock {
        requirePositiveU128(amount, "amount")
        drainPendingCreditsLocked()
        val result = provider.commitRedemption(amount, beneficiary.canonicalPayload())
        val voucher = OfflineCashNoritoV1.decodeRedemptionVoucherShapeExact(result.canonicalEnvelope())
        require(voucher.statement.amount == amount && voucher.statement.beneficiary == beneficiary)
        installAuthoritativeState(result.aggregateState())
        voucher
    }

    /** Recover one byte-identical terminal redemption voucher. */
    fun recoverRedemption(redemptionId: ByteArray): OfflineCashRedemptionVoucherV1? {
        val expected = fixed32(redemptionId, "redemptionId")
        val canonical = provider.recoverRedemption(expected) ?: return null
        val voucher = OfflineCashNoritoV1.decodeRedemptionVoucherShapeExact(canonical)
        require(voucher.statement.redemptionId().contentEquals(expected))
        return voucher
    }

    /** Rotate the complete private balance and replay root in qualified hardware. */
    fun rotateHardwareEpoch(): OfflineCashAggregateStateCommitmentV1 = transitionLock.withLock {
        drainPendingCreditsLocked()
        installAuthoritativeState(provider.rotateHardwareEpoch())
        currentQualification = requireQualified(provider.qualification())
        requireStateQualification(currentAggregateState, currentQualification)
        currentAggregateState
    }

    private fun foldSnapshotLocked(): Int {
        val watermark = provider.pendingCreditWatermark()
        return foldBatchAtWatermarkLocked(watermark)
    }

    private fun drainPendingCreditsLocked(): BigInteger {
        val watermark = provider.pendingCreditWatermark()
        require(watermark.signum() >= 0)
        var total = BigInteger.ZERO
        while (true) {
            val folded = foldBatchAtWatermarkLocked(watermark)
            if (folded == 0) return total
            total = total + BigInteger.valueOf(folded.toLong())
        }
    }

    private fun foldBatchAtWatermarkLocked(watermark: BigInteger): Int {
        require(watermark.signum() >= 0)
        val before = currentJournalRevision
        val beforeCommitment = currentAggregateState.stateCommitment()
        val folded = provider.foldPendingCreditBatch(watermark, 16)
        val successorBytes = folded.aggregateState()
        if (folded.foldedCredits > 0) {
            require(successorBytes != null) { "a non-empty fold batch did not return its successor" }
            installAuthoritativeState(successorBytes)
            require(!currentAggregateState.stateCommitment().contentEquals(beforeCommitment)) {
                "fixed-shape fold batch made no aggregate-state progress"
            }
        }
        val after = provider.journalRevision()
        val expectedRevision = if (folded.foldedCredits == 0) before else before + BigInteger.ONE
        require(after == expectedRevision) {
            "fixed-shape fold batch did not consume exactly one journal revision"
        }
        currentJournalRevision = after
        return folded.foldedCredits
    }

    private fun installAuthoritativeState(bytes: ByteArray) {
        val state = OfflineCashNoritoV1.decodeAggregateStateShapeExact(bytes)
        requireSameAsset(currentAggregateState, state)
        currentAggregateState = state
        currentJournalRevision = provider.journalRevision()
    }

    companion object {
        /** Open only after the complete native/hardware contract and recovery succeed. */
        @JvmStatic
        fun open(provider: OfflineCashHardwareProviderV1): OfflineCashWalletV1 {
            val qualification = requireQualified(provider.qualification())
            val recovery = provider.recover()
            val stateBytes = recovery.aggregateState() ?: provider.bootstrapState()
            val state = OfflineCashNoritoV1.decodeAggregateStateShapeExact(stateBytes)
            requireStateQualification(state, qualification)
            require(provider.journalRevision() == recovery.journalRevision)
            return OfflineCashWalletV1(provider, qualification, state, recovery.journalRevision)
        }

        private fun requireQualified(
            qualification: OfflineCashHardwareQualificationV1,
        ): OfflineCashHardwareQualificationV1 {
            qualification.requireProductionReady()
            return qualification
        }

        private fun requireStateQualification(
            state: OfflineCashAggregateStateCommitmentV1,
            qualification: OfflineCashHardwareQualificationV1,
        ) {
            require(state.releaseId().contentEquals(qualification.releaseId()))
            require(state.networkId == qualification.credential.networkId)
            require(state.keyReference().contentEquals(qualification.credential.deviceKeyReference()))
            require(state.hardwarePolicyId().contentEquals(qualification.profile.hardwareProfileId()))
        }

        private fun requireStateRequestBinding(
            state: OfflineCashAggregateStateCommitmentV1,
            request: OfflineCashPaymentRequestV1,
        ) {
            require(request.releaseId().contentEquals(state.releaseId()))
            require(request.networkId == state.networkId)
            require(request.asset == state.asset && request.assetIncarnation == state.assetIncarnation)
            require(request.scale == state.scale)
            require(request.liabilityPoolId().contentEquals(state.liabilityPoolId()))
        }

        private fun requireSameAsset(
            before: OfflineCashAggregateStateCommitmentV1,
            after: OfflineCashAggregateStateCommitmentV1,
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
