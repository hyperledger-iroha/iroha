// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.util.Collections
import java.util.EnumSet
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock

/** Hardware properties required together by every production KAGEMUSHA V1 provider. */
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
    OFFLINE_HARDWARE_EPOCH_ROTATION,
    ROLLBACK_SAFE_COUNTER_ROLLOVER,
    NO_SOFTWARE_FALLBACK,
}

/** Authenticated release and hardware credential returned by the shared native core. */
class KagemushaHardwareQualificationV1(
    @JvmField val protocolVersion: Int,
    @JvmField val profile: KagemushaHardwareProfileV1,
    @JvmField val credential: KagemushaHardwareCredentialV1,
    releaseId: ByteArray,
    hardwarePolicyDigest: ByteArray,
    coreAuthorizationKeyReference: ByteArray,
    capabilities: Set<KagemushaHardwareCapabilityV1>,
) {
    private val releaseIdValue = fixed32(releaseId, "releaseId")
    private val hardwarePolicy = fixed32(hardwarePolicyDigest, "hardwarePolicyDigest")
    private val coreAuthorizationKey = fixed32(
        coreAuthorizationKeyReference,
        "coreAuthorizationKeyReference",
    )
    private val capabilityValues = if (capabilities.isEmpty()) {
        EnumSet.noneOf(KagemushaHardwareCapabilityV1::class.java)
    } else {
        EnumSet.copyOf(capabilities)
    }

    /** Return the authenticated release identity selected by native core. */
    fun releaseId(): ByteArray = releaseIdValue.copyOf()

    /** Return the authenticated policy-registry root independently bound to aggregate state. */
    fun hardwarePolicyDigest(): ByteArray = hardwarePolicy.copyOf()

    /** Return the governed Core-to-hardware authorization verifier-key reference. */
    fun coreAuthorizationKeyReference(): ByteArray = coreAuthorizationKey.copyOf()

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
            "KAGEMUSHA V1 requires the complete non-forking hardware capability set"
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

/** One exact `MintFold` or `ReceiveFold` transition and the staged credit it consumes. */
class KagemushaHardwareReceiveFoldV1(
    aggregateState: ByteArray,
    @JvmField val selector: KagemushaPendingCreditSelectorV1,
) {
    private val aggregateStateValue = aggregateState.copyOf()

    init {
        require(aggregateStateValue.isNotEmpty())
    }

    fun aggregateState(): ByteArray = aggregateStateValue.copyOf()
}

/** Public result of installing one authenticated pending credit. */
class KagemushaReceiveFoldResultV1(
    @JvmField val aggregateState: KagemushaAggregateStateCommitmentV1,
    @JvmField val selector: KagemushaPendingCreditSelectorV1,
) {
    fun creditId(): ByteArray = selector.creditId()
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

    /** Persist and echo an already caller-persisted exact receiver-request intent. */
    fun reservePaymentRequestOperationId(
        operationId: ByteArray,
        recipientAccount: ByteArray,
        amount: BigInteger,
        validityWindowMillis: Long,
    ): ByteArray

    /** Create and sign the exact caller-persisted request using hardware trusted time. */
    fun createPaymentRequest(
        operationId: ByteArray,
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

    /** Select one deterministic staged credit under an epoch-qualified finite pass. */
    fun selectPendingCredit(
        watermark: KagemushaPendingCreditWatermarkV1?,
        target: KagemushaPendingCreditTargetV1,
    ): KagemushaPendingCreditSelectionV1

    /** Return the monetary-state journal revision, separate from accepted-credit inbox revisions. */
    fun journalRevision(): BigInteger

    /** Fold exactly one staged credit selected by its globally unique identity. */
    fun foldPendingCredit(selector: KagemushaPendingCreditSelectorV1): KagemushaHardwareReceiveFoldV1

    /** Persist and echo the identity already saved with the complete caller payment intent. */
    fun reservePaymentOperationId(operationId: ByteArray, canonicalRequest: ByteArray): ByteArray

    /** Commit one payment under the exact caller-persisted operation identity. */
    fun commitPayment(
        operationId: ByteArray,
        canonicalRequest: ByteArray,
    ): KagemushaHardwareTerminalResultV1

    /** Recover one byte-identical payment from the authenticated durable retry outbox. */
    fun recoverPayment(creditId: ByteArray): ByteArray?

    /** Recover a payment before its terminal credit ID was exposed to the caller. */
    fun recoverPaymentByOperationId(operationId: ByteArray, canonicalRequest: ByteArray): ByteArray?

    /** Persist and echo the identity already saved with the complete caller mint intent. */
    fun reserveMintOperationId(
        operationId: ByteArray,
        amount: BigInteger,
        payerAccount: ByteArray,
        recipientAccount: ByteArray,
    ): ByteArray

    /** Produce the complete hardware-owned mint authorization and encrypted credit. */
    fun prepareMintConstructionBundle(
        operationId: ByteArray,
        amount: BigInteger,
        payerAccount: ByteArray,
        recipientAccount: ByteArray,
    ): KagemushaMintConstructionBundleV1

    /** Recover the complete mint bundle byte-identically. */
    fun recoverMintConstructionBundle(operationId: ByteArray): KagemushaMintConstructionBundleV1?

    /** Verify and record an acknowledgement before releasing the matching outbox entry. */
    fun recordAcknowledgement(
        creditId: ByteArray,
        canonicalRequest: ByteArray,
        canonicalPayment: ByteArray,
        canonicalAcknowledgement: ByteArray,
    )

    /** Persist and echo the identity already saved with the complete caller redemption intent. */
    fun reserveRedemptionOperationId(operationId: ByteArray, amount: BigInteger, beneficiaryAccount: ByteArray): ByteArray

    /** Commit one redemption under the exact caller-persisted operation identity. */
    fun commitRedemption(
        operationId: ByteArray,
        amount: BigInteger,
        beneficiaryAccount: ByteArray,
    ): KagemushaHardwareTerminalResultV1

    /** Recover one byte-identical redemption voucher. */
    fun recoverRedemption(redemptionId: ByteArray): ByteArray?

    /** Recover a redemption before its terminal ID was exposed to the caller. */
    fun recoverRedemptionByOperationId(operationId: ByteArray): ByteArray?

    /** Rotate the complete private aggregate state and replay root to a qualified epoch. */
    fun rotateHardwareEpoch(): ByteArray
}

/** Aggregate-balance KAGEMUSHA V1 orchestration over the authoritative native boundary. */
class KagemushaWalletV1 private constructor(
    private val provider: KagemushaHardwareProviderV1,
    initialQualification: KagemushaHardwareQualificationV1,
    initialState: KagemushaAggregateStateCommitmentV1,
    initialJournalRevision: BigInteger,
) {
    // A drain releases this fair gate after each batch so queued foreground work can proceed.
    private val transitionLock = ReentrantLock(true)

    private class HostSnapshot(
        val qualification: KagemushaHardwareQualificationV1,
        val aggregateState: KagemushaAggregateStateCommitmentV1,
        val journalRevision: BigInteger,
    )

    private class RecoverySnapshot(
        val host: HostSnapshot,
        val recovery: KagemushaHardwareRecoveryV1,
    )

    // Publish a complete validated host projection in one write, including epoch rotation.
    @Volatile
    private var currentSnapshot = HostSnapshot(initialQualification, initialState, initialJournalRevision)
    private val currentQualification get() = currentSnapshot.qualification
    private val currentAggregateState get() = currentSnapshot.aggregateState
    private val currentJournalRevision get() = currentSnapshot.journalRevision

    /** Return the current authenticated compact hardware credential. */
    fun hardwareCredential(): KagemushaHardwareCredentialV1 = currentQualification.credential

    /** Return the latest native-authoritative aggregate-state commitment. */
    fun aggregateState(): KagemushaAggregateStateCommitmentV1 = currentAggregateState

    /** Return the latest rollback-resistant journal revision. */
    fun journalRevision(): BigInteger = currentJournalRevision

    /** Recover interrupted work and refresh authoritative state. */
    fun recover(): KagemushaHardwareRecoveryV1 = transitionLock.withLock {
        val recovered = recoverAuthoritativeSnapshot(provider, allowBootstrap = false)
        val next = recovered.host
        val previous = currentSnapshot
        requireSameAsset(previous.aggregateState, next.aggregateState, includingRelease = false)
        require(
            next.qualification.credential.laneCommitment().contentEquals(
                previous.qualification.credential.laneCommitment(),
            ),
        ) { "recovery changed the wallet identity" }
        val previousGeneration = previous.qualification.credential.hardwareEpochGeneration
        val recoveredGeneration = next.qualification.credential.hardwareEpochGeneration
        if (next.aggregateState.hardwareEpochId().contentEquals(previous.aggregateState.hardwareEpochId())) {
            require(recoveredGeneration == previousGeneration)
            require(next.aggregateState.keyReference().contentEquals(previous.aggregateState.keyReference()))
            require(next.journalRevision >= previous.journalRevision) { "recovery rolled back durable state" }
            require(
                next.journalRevision != previous.journalRevision ||
                    KagemushaNoritoV1.encodeAggregateStateShape(next.aggregateState).contentEquals(
                        KagemushaNoritoV1.encodeAggregateStateShape(previous.aggregateState),
                    ),
            ) { "recovery equivocated at the same journal revision" }
        } else {
            // Journals are epoch scoped. Native authenticates rotation; reject an old generation.
            require(java.lang.Long.compareUnsigned(recoveredGeneration, previousGeneration) > 0) {
                "recovery did not advance the authenticated hardware epoch"
            }
        }
        currentSnapshot = next
        KagemushaHardwareRecoveryV1(
            KagemushaNoritoV1.encodeAggregateStateShape(next.aggregateState),
            next.journalRevision,
            recovered.recovery.pendingCreditCount,
            recovered.recovery.retryOutboxCount,
        )
    }

    /** Create a signed exact-amount receiver request. */
    fun createPaymentRequest(
        operationId: ByteArray,
        recipient: KagemushaAccountIdV1,
        amount: BigInteger,
        validityWindowMillis: Long,
    ): KagemushaPaymentRequestV1 = transitionLock.withLock {
        requirePositiveU128(amount, "amount")
        require(validityWindowMillis in 1..KagemushaWireV1.REQUEST_MAX_TTL_MS)
        val request = KagemushaNoritoV1.decodePaymentRequestShapeExact(
            provider.createPaymentRequest(fixed32(operationId, "operationId"), recipient.canonicalPayload(), amount, validityWindowMillis),
        )
        require(request.recipient == recipient)
        require(request.requestId().contentEquals(operationId))
        require(request.amount == amount)
        require(request.expiresAtMs - request.issuedAtMs == validityWindowMillis)
        requireStateRequestBinding(currentAggregateState, request)
        request
    }

    /** Reserve only after the caller has durably saved ID, recipient, amount, and validity. */
    fun reservePaymentRequestOperationId(
        operationId: ByteArray,
        recipient: KagemushaAccountIdV1,
        amount: BigInteger,
        validityWindowMillis: Long,
    ): ByteArray {
        requirePositiveU128(amount, "amount")
        require(validityWindowMillis in 1..KagemushaWireV1.REQUEST_MAX_TTL_MS)
        return provider.reservePaymentRequestOperationId(
            fixed32(operationId, "operationId"), recipient.canonicalPayload(), amount, validityWindowMillis
        )
    }

    /** Persist and echo the identity the caller has already saved before beginning a payment. */
    fun reservePaymentOperationId(operationId: ByteArray, request: KagemushaPaymentRequestV1): ByteArray =
        provider.reservePaymentOperationId(fixed32(operationId, "operationId"), KagemushaNoritoV1.encodePaymentRequestShape(request))

    /** Prepare, prove, atomically commit, and return a receiver-bound payment. */
    fun send(
        request: KagemushaPaymentRequestV1,
        operationId: ByteArray,
    ): KagemushaPaymentV1 {
        val operationId = fixed32(operationId, "operationId")
        return transitionLock.withLock {
            foldRequiredCreditsLocked(request.amount)
            val canonicalRequest = KagemushaNoritoV1.encodePaymentRequestShape(request)
            val result = provider.commitPayment(operationId, canonicalRequest)
            val payment = KagemushaNoritoV1.decodePaymentShapeExact(
                result.canonicalEnvelope(),
                request,
            )
            installAuthoritativeState(result.aggregateState())
            payment
        }
    }

    /** Stage a payment and return its durable acknowledgement. */
    fun stagePayment(
        request: KagemushaPaymentRequestV1,
        payment: KagemushaPaymentV1,
    ): KagemushaStagedPaymentV1 = transitionLock.withLock {
        val canonicalRequest = KagemushaNoritoV1.encodePaymentRequestShape(request)
        val canonicalPayment = KagemushaNoritoV1.encodePaymentShape(payment, request)
        val before = readJournalRevision(provider)
        val staged = provider.stagePayment(
            canonicalRequest,
            canonicalPayment,
        )
        require(staged.creditId().contentEquals(payment.output.creditId()))
        val canonicalAcknowledgement = staged.acknowledgement()
        val acknowledgement = KagemushaNoritoV1.decodeAcknowledgementShapeExact(
            canonicalAcknowledgement,
            request,
            payment,
        )
        val after = readJournalRevision(provider)
        // Staging advances the native inbox journal, not the monetary-state journal. Reading
        // before this call also permits an exact retry after the prior durable stage lost its ACK.
        require(after == before) { "inbound staging changed the monetary-state journal" }
        currentSnapshot = HostSnapshot(currentQualification, currentAggregateState, after)
        KagemushaStagedPaymentV1(staged.disposition, acknowledgement, canonicalAcknowledgement)
    }

    /** Reserve and return the ID the caller must persist before mint preparation. */
    fun reserveMintOperationId(
        operationId: ByteArray,
        amount: BigInteger,
        payer: KagemushaAccountIdV1,
        recipient: KagemushaAccountIdV1,
    ): ByteArray {
        requirePositiveU128(amount, "amount")
        return provider.reserveMintOperationId(
            fixed32(operationId, "operationId"),
            amount,
            payer.canonicalPayload(),
            recipient.canonicalPayload(),
        )
    }

    /** Ask hardware for the proof-bearing authorization and exact encrypted credit. */
    fun prepareMintConstructionBundle(
        operationId: ByteArray,
        amount: BigInteger,
        payer: KagemushaAccountIdV1,
        recipient: KagemushaAccountIdV1,
    ): KagemushaMintConstructionBundleV1 {
        requirePositiveU128(amount, "amount")
        return provider.prepareMintConstructionBundle(
            fixed32(operationId, "operationId"),
            amount,
            payer.canonicalPayload(),
            recipient.canonicalPayload(),
        )
    }

    /** Recover the complete hardware-owned mint bundle byte-identically. */
    fun recoverMintConstructionBundle(
        operationId: ByteArray,
    ): KagemushaMintConstructionBundleV1? =
        provider.recoverMintConstructionBundle(fixed32(operationId, "operationId"))

    /** Build the complete reserve-facing request without host-generated ciphertext. */
    fun prepareTopUpRequest(
        operationId: ByteArray,
        amount: BigInteger,
        payer: KagemushaAccountIdV1,
        recipient: KagemushaAccountIdV1,
    ): KagemushaTopUpRequestV1 = prepareMintConstructionBundle(
        operationId,
        amount,
        payer,
        recipient,
    ).topUpRequest(currentQualification.credential)

    /** Stage a finalized reserve-backed mint only with its exact pre-debit authorization. */
    fun stageMintCredit(
        authorization: KagemushaMintAuthorizationV1,
        mintCredit: KagemushaMintCreditV1,
    ): KagemushaHardwareStageDispositionV1 = transitionLock.withLock {
        val canonicalAuthorization = KagemushaNoritoV1.encodeMintAuthorizationShape(authorization)
        val canonicalCredit = KagemushaNoritoV1.encodeMintCreditShape(mintCredit, authorization)
        val before = readJournalRevision(provider)
        val staged = provider.stageMintCredit(canonicalAuthorization, canonicalCredit)
        require(staged.creditId().contentEquals(mintCredit.statement.lifecycle.creditId()))
        val after = readJournalRevision(provider)
        // This is staging only. A subsequent authenticated MintFold consumes monetary state.
        require(after == before) { "mint staging changed the monetary-state journal" }
        currentSnapshot = HostSnapshot(currentQualification, currentAggregateState, after)
        staged.disposition
    }

    /** Fold one authenticated mint or peer selector into the aggregate balance. */
    fun foldPendingCredit(
        selector: KagemushaPendingCreditSelectorV1,
    ): KagemushaReceiveFoldResultV1 = transitionLock.withLock {
        foldCreditLocked(selector)
    }

    /**
     * Drain the staged inbox, releasing the lane after every credit.
     *
     * A concurrent hardware-epoch rotation interrupts this pass with [IllegalStateException];
     * start a new pass to obtain the new epoch's watermark. No old watermark is reused.
     */
    fun drainPendingCredits(): BigInteger {
        val snapshot = transitionLock.withLock {
            Pair(
                currentQualification.credential.hardwareEpochId(),
                currentQualification.credential.hardwareEpochGeneration,
            )
        }
        var total = BigInteger.ZERO
        var watermark: KagemushaPendingCreditWatermarkV1? = null
        while (true) {
            val folded = transitionLock.withLock {
                val credential = currentQualification.credential
                check(
                    credential.hardwareEpochId().contentEquals(snapshot.first) &&
                        credential.hardwareEpochGeneration == snapshot.second,
                ) { "hardware epoch changed during inbox drain; start a new drain pass" }
                val selection = provider.selectPendingCredit(
                    watermark,
                    KagemushaPendingCreditTargetV1.DrainAll,
                )
                watermark?.let {
                    require(it.sameAs(selection.watermark)) {
                        "pending-credit watermark changed during drain"
                    }
                }
                watermark = selection.watermark
                selection.nextPending?.let(::foldCreditLocked)
            } ?: return total
            check(folded.creditId().isNotEmpty())
            total += BigInteger.ONE
        }
    }

    /** Recover a byte-identical exposed payment for transport retry. */
    fun recoverPayment(
        request: KagemushaPaymentRequestV1,
        creditId: ByteArray,
    ): KagemushaPaymentV1? {
        val expected = fixed32(creditId, "creditId")
        val canonical = provider.recoverPayment(expected) ?: return null
        val payment = KagemushaNoritoV1.decodePaymentShapeExact(canonical, request)
        require(payment.output.creditId().contentEquals(expected))
        return payment
    }

    /** Recover after a crash which occurred before the payment credit ID reached the caller. */
    fun recoverPaymentByOperationId(
        request: KagemushaPaymentRequestV1,
        operationId: ByteArray,
    ): KagemushaPaymentV1? {
        val canonicalRequest = KagemushaNoritoV1.encodePaymentRequestShape(request)
        val canonical = provider.recoverPaymentByOperationId(
            fixed32(operationId, "operationId"),
            canonicalRequest,
        ) ?: return null
        val payment = KagemushaNoritoV1.decodePaymentShapeExact(canonical, request)
        require(
            KagemushaNoritoV1.encodePaymentShape(payment, request).contentEquals(canonical),
        ) { "recovered payment envelope is not byte-identical" }
        return payment
    }

    /** Record a shape-valid ACK; native core authenticates it before outbox release. */
    fun recordAcknowledgement(
        request: KagemushaPaymentRequestV1,
        payment: KagemushaPaymentV1,
        acknowledgement: KagemushaAcknowledgementV1,
    ) {
        val canonicalRequest = KagemushaNoritoV1.encodePaymentRequestShape(request)
        val canonicalPayment = KagemushaNoritoV1.encodePaymentShape(payment, request)
        val canonicalAcknowledgement =
            KagemushaNoritoV1.encodeAcknowledgementShape(
                acknowledgement,
                request,
                payment,
            )
        KagemushaNoritoV1.validateCompleteExchangeShape(
            request,
            payment,
            acknowledgement,
        )
        provider.recordAcknowledgement(
            payment.output.creditId(),
            canonicalRequest,
            canonicalPayment,
            canonicalAcknowledgement,
        )
    }

    /** Persist and echo the identity the caller has already saved before redemption. */
    fun reserveRedemptionOperationId(
        operationId: ByteArray,
        amount: BigInteger,
        beneficiary: KagemushaAccountIdV1,
    ): ByteArray {
        requirePositiveU128(amount, "amount")
        return provider.reserveRedemptionOperationId(fixed32(operationId, "operationId"), amount, beneficiary.canonicalPayload())
    }

    /** Prepare, prove, commit, and install one full or partial terminal redemption. */
    fun redeem(
        amount: BigInteger,
        beneficiary: KagemushaAccountIdV1,
        operationId: ByteArray,
    ): KagemushaRedemptionVoucherV1 {
        requirePositiveU128(amount, "amount")
        val operationId = fixed32(operationId, "operationId")
        return transitionLock.withLock {
            foldRequiredCreditsLocked(amount)
            val result = provider.commitRedemption(
                operationId,
                amount,
                beneficiary.canonicalPayload(),
            )
            val voucher = KagemushaNoritoV1.decodeRedemptionVoucherShapeExact(result.canonicalEnvelope())
            require(voucher.statement.amount == amount && voucher.statement.beneficiary == beneficiary)
            installAuthoritativeState(result.aggregateState())
            voucher
        }
    }

    /** Recover one byte-identical terminal redemption voucher. */
    fun recoverRedemption(redemptionId: ByteArray): KagemushaRedemptionVoucherV1? {
        val expected = fixed32(redemptionId, "redemptionId")
        val canonical = provider.recoverRedemption(expected) ?: return null
        val voucher = KagemushaNoritoV1.decodeRedemptionVoucherShapeExact(canonical)
        require(voucher.statement.redemptionId().contentEquals(expected))
        return voucher
    }

    /** Recover after a crash which occurred before the redemption ID reached the caller. */
    fun recoverRedemptionByOperationId(operationId: ByteArray): KagemushaRedemptionVoucherV1? {
        val canonical = provider.recoverRedemptionByOperationId(
            fixed32(operationId, "operationId"),
        ) ?: return null
        return KagemushaNoritoV1.decodeRedemptionVoucherShapeExact(canonical)
    }

    /**
     * Rotate the complete private balance, replay root, and pending inbox in qualified hardware.
     * Do not fold first: rotation must remain available when the old epoch's counters are full.
     */
    fun rotateHardwareEpoch(): KagemushaAggregateStateCommitmentV1 = transitionLock.withLock {
        val previousState = currentAggregateState
        val previousCredential = currentQualification.credential
        // Long carries the canonical u64 bits; -1 is the exhausted generation, not Long.MAX_VALUE.
        require(previousCredential.hardwareEpochGeneration != -1L)
        val state = KagemushaNoritoV1.decodeAggregateStateShapeExact(provider.rotateHardwareEpoch())
        val qualification = requireQualified(provider.qualification())
        val credential = qualification.credential
        val revision = readJournalRevision(provider)
        requireSameAsset(previousState, state)
        requireStateQualification(state, qualification)
        require(credential.networkId == previousCredential.networkId)
        require(credential.laneCommitment().contentEquals(previousCredential.laneCommitment()))
        require(credential.hardwareEpochGeneration == previousCredential.hardwareEpochGeneration + 1L)
        require(!credential.hardwareEpochId().contentEquals(previousCredential.hardwareEpochId()))
        require(!state.stateCommitment().contentEquals(previousState.stateCommitment()))
        require(state.sequence == BigInteger.ZERO && revision == BigInteger.ZERO) {
            "hardware rotation must reset the new epoch's state and journal counters"
        }
        // Publish the new host snapshot only after every returned binding has been checked.
        currentSnapshot = HostSnapshot(qualification, state, revision)
        state
    }

    private fun foldCreditLocked(
        selector: KagemushaPendingCreditSelectorV1,
    ): KagemushaReceiveFoldResultV1 {
        val before = currentJournalRevision
        val previousState = currentAggregateState
        val hardwareFold = provider.foldPendingCredit(selector)
        require(hardwareFold.selector.kind == selector.kind)
        require(hardwareFold.selector.creditId().contentEquals(selector.creditId()))
        val successor = KagemushaNoritoV1.decodeAggregateStateShapeExact(hardwareFold.aggregateState())
        requireSameAsset(previousState, successor)
        requireStateQualification(successor, currentQualification)
        require(successor.sequence == previousState.sequence + BigInteger.ONE) {
            "receive fold did not consume exactly one logical sequence"
        }
        require(!successor.stateCommitment().contentEquals(previousState.stateCommitment())) {
            "receive fold made no aggregate-state progress"
        }
        val after = readJournalRevision(provider)
        require(after == before + BigInteger.ONE) {
            "receive fold did not consume exactly one journal revision"
        }
        currentSnapshot = HostSnapshot(currentQualification, successor, after)
        return KagemushaReceiveFoldResultV1(successor, selector)
    }

    /** Drain the provider-visible mixed mint/peer inbox without an item-count ceiling. */
    private fun drainPendingCreditsLocked() {
        var watermark: KagemushaPendingCreditWatermarkV1? = null
        while (true) {
            val selection = provider.selectPendingCredit(
                watermark,
                KagemushaPendingCreditTargetV1.DrainAll,
            )
            watermark?.let { require(it.sameAs(selection.watermark)) }
            watermark = selection.watermark
            val selector = selection.nextPending ?: return
            foldCreditLocked(selector)
        }
    }

    private fun foldRequiredCreditsLocked(requiredBalance: BigInteger) {
        while (true) {
            val selection = provider.selectPendingCredit(
                null,
                KagemushaPendingCreditTargetV1.RequiredBalance(requiredBalance),
            )
            val selector = selection.nextPending ?: return
            foldCreditLocked(selector)
        }
    }

    private fun installAuthoritativeState(bytes: ByteArray) {
        val state = KagemushaNoritoV1.decodeAggregateStateShapeExact(bytes)
        requireSameAsset(currentAggregateState, state)
        requireStateQualification(state, currentQualification)
        val revision = readJournalRevision(provider)
        currentSnapshot = HostSnapshot(currentQualification, state, revision)
    }

    companion object {
        /** Open only after the complete native/hardware contract and recovery succeed. */
        @JvmStatic
        fun open(provider: KagemushaHardwareProviderV1): KagemushaWalletV1 {
            val recovered = recoverAuthoritativeSnapshot(provider, allowBootstrap = true).host
            return KagemushaWalletV1(
                provider,
                recovered.qualification,
                recovered.aggregateState,
                recovered.journalRevision,
            )
        }

        private fun recoverAuthoritativeSnapshot(
            provider: KagemushaHardwareProviderV1,
            allowBootstrap: Boolean,
        ): RecoverySnapshot {
            requireQualified(provider.qualification())
            var recovery = provider.recover()
            // Recovery can finish a committed rotation before returning its snapshot.
            var qualification = requireQualified(provider.qualification())
            var stateBytes = recovery.aggregateState()
            if (stateBytes == null) {
                require(allowBootstrap) { "recovery lost an existing aggregate state" }
                val bootstrapped = provider.bootstrapState().copyOf()
                // A successful return is not proof that bootstrap was durably installed.
                // Observe its persisted state and revision together, without repeating bootstrap.
                recovery = provider.recover()
                qualification = requireQualified(provider.qualification())
                stateBytes = recovery.aggregateState()
                require(stateBytes != null && stateBytes.contentEquals(bootstrapped)) {
                    "bootstrap differs from the authoritative recovery snapshot"
                }
            }
            val state = KagemushaNoritoV1.decodeAggregateStateShapeExact(stateBytes)
            requireStateQualification(state, qualification)
            require(readJournalRevision(provider) == recovery.journalRevision)
            return RecoverySnapshot(
                HostSnapshot(qualification, state, recovery.journalRevision),
                recovery,
            )
        }

        private fun requireQualified(
            qualification: KagemushaHardwareQualificationV1,
        ): KagemushaHardwareQualificationV1 {
            qualification.requireProductionReady()
            return qualification
        }

        private fun readJournalRevision(provider: KagemushaHardwareProviderV1): BigInteger =
            provider.journalRevision().also { requireUnsigned128(it, "journalRevision") }

        private fun requireStateQualification(
            state: KagemushaAggregateStateCommitmentV1,
            qualification: KagemushaHardwareQualificationV1,
        ) {
            require(state.releaseId().contentEquals(qualification.releaseId()))
            require(state.networkId == qualification.credential.networkId)
            require(state.hardwareEpochId().contentEquals(qualification.credential.hardwareEpochId()))
            require(state.keyReference().contentEquals(qualification.credential.deviceKeyReference()))
            require(state.hardwarePolicyId().contentEquals(qualification.hardwarePolicyDigest()))
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
            includingRelease: Boolean = true,
        ) {
            require(after.version == before.version)
            if (includingRelease) require(after.releaseId().contentEquals(before.releaseId()))
            require(after.networkId == before.networkId)
            require(after.asset == before.asset && after.assetIncarnation == before.assetIncarnation)
            require(after.scale == before.scale)
            require(after.liabilityPoolId().contentEquals(before.liabilityPoolId()))
            require(after.laneId().contentEquals(before.laneId()))
        }

    }
}
