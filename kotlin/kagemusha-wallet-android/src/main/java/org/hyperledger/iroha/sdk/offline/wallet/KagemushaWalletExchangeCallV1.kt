// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import java.io.Closeable

/** Work admitted synchronously before UI/actor/worker enqueue. Finish after its last result.
 * Aliases share the same task fence; finishing never abandons an already running Native call. */
class KagemushaWalletExchangeCallV1 internal constructor(
    private val wallet: KagemushaWalletV1,
    private val exchange: KagemushaWalletExchangeV1,
    private val task: KagemushaWalletExchangeTaskV1,
) : Closeable {
    private fun handle(): Long = wallet.exchangeHandle()
    private fun setup(input: KagemushaWalletSetupInputV1) = wallet.setup(input, exchange.token)
    private fun deletionSetup(input: KagemushaWalletSetupInputV1) = wallet.deletionSetup(input, exchange.token)
    private val deletionGate get() = wallet.deletionGate
    /** End this accepted task without joining or holding a Native lock. */
    fun finish() = task.close()
    override fun close() = finish()

    /** Display native-selected review DATA before explicit destructive confirmation. */
    fun reviewCustodyDeletion(): KagemushaWalletDeletionReviewV1 = task.useCall { reviewCustodyDeletionAccepted() }
    private fun reviewCustodyDeletionAccepted(): KagemushaWalletDeletionReviewV1 = deletionGate.review(exchange) { setup(KagemushaWalletSetupInputV1(48)) }

    /** Permanently destroy payment-key custody. Display the review warning first.
     * Any error freezes ordinary calls until recovery proves a definitive outcome. */
    fun destructivelyDeleteCustody(review: KagemushaWalletDeletionReviewV1): ByteArray = task.useCall { destructivelyDeleteCustodyAccepted(review) }
    private fun destructivelyDeleteCustodyAccepted(review: KagemushaWalletDeletionReviewV1): ByteArray =
        deletionGate.confirm(review, exchange) { token -> deletionSetup(KagemushaWalletSetupInputV1(49, token = token)) }

    /** Complete restart-safe native enrollment Bootstrap. */
    fun bootstrap(): KagemushaWalletCallV1 = task.useCall { bootstrapAccepted() }
    private fun bootstrapAccepted(): KagemushaWalletCallV1 = setup(KagemushaWalletSetupInputV1(0)).completion()

    /** Freeze the exact next Load instruction under durable Native request custody.
     * Retry the same request ID and amount after uncertain delivery. */
    fun prepareLedgerLoad(requestId: ByteArray, amount: KagemushaWalletUInt128V1): ByteArray = task.useCall { prepareLedgerLoadAccepted(requestId, amount) }
    private fun prepareLedgerLoadAccepted(requestId: ByteArray, amount: KagemushaWalletUInt128V1): ByteArray {
        val result = setup(KagemushaWalletSetupInputV1(27, identity = requestId, amount = amount))
        if (result.status != KagemushaWalletCallV1.LEDGER_INSTRUCTION) throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        return result.bytes()
    }

    /** Verify one direct ledger finality original for this exact signed Unload envelope. */
    fun ingestUnloadFinality(claim: ByteArray, signedWire: ByteArray,
        finality: ByteArray): KagemushaWalletLedgerIngestV1 = task.useCall {
        ingestUnloadFinalityAccepted(claim, signedWire, finality)
    }
    private fun ingestUnloadFinalityAccepted(claim: ByteArray, signedWire: ByteArray,
        finality: ByteArray): KagemushaWalletLedgerIngestV1 =
        KagemushaWalletLedgerIngestV1.of(setup(KagemushaWalletSetupInputV1(34,
            first = claim, second = signedWire, third = finality)))

    /** Durably admit this exact existing-account-signed outer Activate before its first POST.
     * An already retained attempt may report family confirmation from another retained attempt;
     * it is not this outer's execution result. */
    fun retainActivationAttempt(signedWire: ByteArray): KagemushaWalletLedgerOutcomeV1 = task.useCall {
        KagemushaWalletLedgerOutcomeV1.of(setup(KagemushaWalletSetupInputV1(46, first = signedWire)))
    }

    /** Verify one direct ledger finality original for this retained Activate attempt. */
    fun ingestActivationFinality(signedWire: ByteArray, finality: ByteArray): KagemushaWalletLedgerIngestV1 = task.useCall {
        ingestActivationFinalityAccepted(signedWire, finality)
    }
    private fun ingestActivationFinalityAccepted(signedWire: ByteArray, finality: ByteArray): KagemushaWalletLedgerIngestV1 =
        KagemushaWalletLedgerIngestV1.of(setup(KagemushaWalletSetupInputV1(36, first = signedWire, second = finality)))

    /** Confirm one retained attempt with at most one finality read. An unselected epoch advances
     * at most [maximumBoundaries] boundaries before retrying the same in-memory original once.
     * The accepted exchange retains every asynchronous continuation until it settles. */
    fun confirmLedgerAttempt(
        attempt: KagemushaWalletLedgerAttemptV1, schemeId: ByteArray, walletId: ByteArray,
        transport: org.hyperledger.iroha.sdk.client.HttpClientTransport,
        canonicalAuth: org.hyperledger.iroha.sdk.client.ToriiCanonicalRequestAuth,
        requireCurrentOwner: Runnable, maximumBoundaries: Int = 64,
    ): java.util.concurrent.CompletableFuture<KagemushaWalletLedgerConfirmationV1> = task.useFuture { continuation ->
        val retainedScheme = schemeId.copyOf()
        val retainedWallet = walletId.copyOf()
        val currentOwner = Runnable { continuation.useCall { requireCurrentOwner.run() } }
        fun selection(entrypoint: ByteArray) =
            org.hyperledger.iroha.sdk.client.ToriiKagemushaWalletLedgerSelectionV1(retainedScheme, retainedWallet, entrypoint)
        confirmKagemushaLedgerAttemptV1(currentOwner,
            { continuation.useCall { ledgerAttemptOutcome(attempt) } },
            { entrypoint -> continuation.useCall {
                transport.getKagemushaWalletLedgerAttemptFinalityV1(selection(entrypoint), canonicalAuth, currentOwner)
            } },
            { finality -> continuation.useCall { ingestLedgerAttemptAccepted(attempt, finality) } },
            { entrypoint, targetHeight -> continuation.useCall {
                val selected = selection(entrypoint)
                synchronizeKagemushaLedgerEpochsV1(targetHeight, maximumBoundaries, currentOwner,
                    { continuation.useCall { wallet.epochProgress() } },
                    { height -> continuation.useCall {
                        transport.getKagemushaWalletLedgerAttemptEpochV1(selected, height, canonicalAuth, currentOwner)
                    } },
                    { epoch, bytes -> continuation.useCall { ingestEpochBoundaryAccepted(epoch, bytes) } })
            } })
    }
    private fun ledgerAttemptOutcome(attempt: KagemushaWalletLedgerAttemptV1): KagemushaWalletLedgerOutcomeV1 = when (attempt) {
        is KagemushaWalletLedgerAttemptV1.Activation -> wallet.activationAttemptOutcome(attempt.signedWire())
        is KagemushaWalletLedgerAttemptV1.Unload -> wallet.unloadAttemptOutcome(attempt.claim(), attempt.signedWire())
    }
    private fun ingestLedgerAttemptAccepted(attempt: KagemushaWalletLedgerAttemptV1, finality: ByteArray): KagemushaWalletLedgerIngestV1 = when (attempt) {
        is KagemushaWalletLedgerAttemptV1.Activation -> ingestActivationFinalityAccepted(attempt.signedWire(), finality)
        is KagemushaWalletLedgerAttemptV1.Unload -> ingestUnloadFinalityAccepted(attempt.claim(), attempt.signedWire(), finality)
    }

    /** Exact retained Activate frame for ledger submission; this is not activation confirmation. */
    fun activation(): ByteArray = task.useCall { activationAccepted() }
    private fun activationAccepted(): ByteArray {
        val result = setup(KagemushaWalletSetupInputV1(15))
        if (result.status != KagemushaWalletCallV1.ACTIVATION) throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        return result.bytes()
    }

    /** Exact native signed CloseLoads frame for ledger submission, not closure confirmation.
     * Reuse requestId for exact retries; use a fresh id after a preissued Load and new complete consumer. */
    fun closeLoads(requestId: ByteArray): ByteArray = task.useCall { closeLoadsAccepted(requestId) }
    private fun closeLoadsAccepted(requestId: ByteArray): ByteArray {
        val result = setup(KagemushaWalletSetupInputV1(19, identity = requestId))
        if (result.status != KagemushaWalletCallV1.CLOSE_LOADS) throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        return result.bytes()
    }

    /** Verify one bounded next native Sumeragi proof and durably select its prefix. */
    fun ingestLedgerFinality(original: ByteArray): KagemushaWalletLedgerProgressV1 = task.useCall { ingestLedgerFinalityAccepted(original) }
    private fun ingestLedgerFinalityAccepted(original: ByteArray): KagemushaWalletLedgerProgressV1 =
        KagemushaWalletLedgerProgressV1(setup(KagemushaWalletSetupInputV1(23, first = original)))

    /** Verify one exact incumbent boundary and durably retain its successor. */
    fun ingestEpochBoundary(expectedEpoch: java.math.BigInteger, original: ByteArray): KagemushaWalletEpochProgressV1 = task.useCall { ingestEpochBoundaryAccepted(expectedEpoch, original) }
    private fun ingestEpochBoundaryAccepted(expectedEpoch: java.math.BigInteger, original: ByteArray): KagemushaWalletEpochProgressV1 {
        require(expectedEpoch.signum() >= 0 && expectedEpoch.bitLength() <= 64) { "u64 epoch required" }
        return KagemushaWalletEpochProgressV1(setup(KagemushaWalletSetupInputV1(53,
            amount = KagemushaWalletUInt128V1(expectedEpoch.toLong(), 0), first = original)))
    }

    /** Synchronize at most 64 durable epoch boundaries for the exact receipt, without crediting value.
     * Cancellation and limits preserve progress. Repeat while boundaryHeight is below the receipt
     * height; ordinary load independently verifies and refuses a missing epoch afterward. */
    fun synchronizeLoadEpochs(original: KagemushaWalletLoadOriginalV1,
        transport: org.hyperledger.iroha.sdk.client.HttpClientTransport,
        canonicalAuth: org.hyperledger.iroha.sdk.client.ToriiCanonicalRequestAuth,
        requireCurrentOwner: Runnable, maximumBoundaries: Int = 64,
    ): java.util.concurrent.CompletableFuture<KagemushaWalletEpochProgressV1> = task.useFuture { continuation ->
        synchronizeKagemushaEpochsV1(original.blockHeight, maximumBoundaries,
            Runnable { continuation.useCall { requireCurrentOwner.run() } },
            { continuation.useCall { wallet.epochProgress() } },
            { height -> continuation.useCall {
                transport.getKagemushaWalletLoadEpochOriginalV1(original.selection, height,
                    canonicalAuth, Runnable { continuation.useCall { requireCurrentOwner.run() } })
            } },
            { epoch, bytes -> continuation.useCall { ingestEpochBoundaryAccepted(epoch, bytes) } })
    }

    /** Native authenticates exact World and payout-row originals against its selected tip. */
    fun acknowledgeFeePayout(creditId: ByteArray, world: ByteArray, payout: ByteArray) = task.useCall { acknowledgeFeePayoutAccepted(creditId, world, payout) }
    private fun acknowledgeFeePayoutAccepted(creditId: ByteArray, world: ByteArray, payout: ByteArray) {
        val result = setup(KagemushaWalletSetupInputV1(25, identity = creditId, first = world, second = payout))
        if (result.status != 35) throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
    }

    /** Native issues the open Request for [amount] from its selected policy and retains it by digest;
     * a retry of this identity returns its exact original bytes. Payer wallet, payer account and send
     * ordinal are zero: the receiver signature covers only the terms, and the payer completes them. */
    fun request(setupId: ByteArray, amount: KagemushaWalletUInt128V1, feeSchedule: ByteArray? = null, feeCertificate: ByteArray? = null): ByteArray = task.useCall { requestAccepted(setupId, amount, feeSchedule, feeCertificate) }
    private fun requestAccepted(setupId: ByteArray, amount: KagemushaWalletUInt128V1, feeSchedule: ByteArray? = null, feeCertificate: ByteArray? = null): ByteArray {
        require((feeSchedule == null) == (feeCertificate == null)) { "fee originals must be paired" }
        if (feeSchedule != null) require(feeSchedule.isNotEmpty() && feeCertificate!!.isNotEmpty()) { "fee originals must not be empty" }
        return setup(KagemushaWalletSetupInputV1(2, setupId, amount,
            second = feeSchedule ?: byteArrayOf(), third = feeCertificate ?: byteArrayOf())).original()
    }

    /** Exact signed fee DATA feeds the ordinary Native open Request for [amount]. Null is accepted only
     * by Native's zero-fee rule; never retry a failed nonzero-fee Request with missing originals. */
    fun requestWithFeePolicy(setupId: ByteArray, amount: KagemushaWalletUInt128V1, scheduleEnvelope: ByteArray?, certificatesEnvelope: ByteArray?): ByteArray = task.useCall { requestWithFeePolicyAccepted(setupId, amount, scheduleEnvelope, certificatesEnvelope) }
    private fun requestWithFeePolicyAccepted(setupId: ByteArray, amount: KagemushaWalletUInt128V1, scheduleEnvelope: ByteArray?, certificatesEnvelope: ByteArray?): ByteArray {
        require((scheduleEnvelope == null) == (certificatesEnvelope == null)) { "fee PolicyData must be paired" }
        if (scheduleEnvelope != null) require(scheduleEnvelope.isNotEmpty() && certificatesEnvelope!!.isNotEmpty())
        return setup(KagemushaWalletSetupInputV1(39, setupId, amount,
            first = scheduleEnvelope ?: byteArrayOf(), second = certificatesEnvelope ?: byteArrayOf())).original()
    }

    /** Only the native nonce leaves clock custody; this token has no serialized form. */
    fun beginTimeExchange(): KagemushaWalletTimeExchangeV1 = task.useCall { beginTimeExchangeAccepted() }
    private fun beginTimeExchangeAccepted(): KagemushaWalletTimeExchangeV1 =
        setup(KagemushaWalletSetupInputV1(4)).exchange(exchange)

    /** Consume the native token and retain the verified response before TimeAnchor refresh. */
    fun finishTimeExchange(exchange: KagemushaWalletTimeExchangeV1, anchor: ByteArray, certificate: ByteArray) = task.useCall { finishTimeExchangeAccepted(exchange, anchor, certificate) }
    private fun finishTimeExchangeAccepted(exchange: KagemushaWalletTimeExchangeV1, anchor: ByteArray, certificate: ByteArray) {
        // Copy and bound the originals before consuming this owner's one-use challenge.
        val input = KagemushaWalletSetupInputV1(5, token = exchange.tokenFor(this.exchange), first = anchor, second = certificate)
        handle()
        exchange.consume(this.exchange)
        setup(input).timeRetained()
    }

    /** Load the exact ordinary-ledger receipt and native BLS finality evidence. */
    fun load(requestId: ByteArray, receipt: ByteArray, finality: ByteArray): KagemushaWalletCallV1 = task.useCall { loadAccepted(requestId, receipt, finality) }
    private fun loadAccepted(requestId: ByteArray, receipt: ByteArray, finality: ByteArray): KagemushaWalletCallV1 =
        execute(KagemushaWalletOperationInputV1(requestId, 0, first = receipt, second = finality))

    /** Authenticate Send and its fully folded source before fresh hardware UI confirmation. */
    fun reviewSend(requestId: ByteArray, request: ByteArray, destinationAccountOriginal: ByteArray,
        expectation: KagemushaWalletReviewExpectationV1): KagemushaWalletReviewDecisionV1 = task.useCall {
        reviewSendAccepted(requestId, request, destinationAccountOriginal, expectation)
    }
    private fun reviewSendAccepted(requestId: ByteArray, request: ByteArray, destinationAccountOriginal: ByteArray,
        expectation: KagemushaWalletReviewExpectationV1): KagemushaWalletReviewDecisionV1 {
        require(request.isNotEmpty() && request.size <= 10_034)
        require(destinationAccountOriginal.isNotEmpty() && destinationAccountOriginal.size <= 4_096)
        return review(KagemushaWalletReviewInputV1(requestId, 1, expectation, first = request, second = destinationAccountOriginal),
            KagemushaWalletReviewProjectionV1.Kind.SEND)
    }

    /** Receive selects the durably issued Request locally by the Payment's digest. */
    fun receive(requestId: ByteArray, payment: ByteArray, payerCredential: ByteArray, certificates: ByteArray): KagemushaWalletCallV1 = task.useCall { receiveAccepted(requestId, payment, payerCredential, certificates) }
    private fun receiveAccepted(requestId: ByteArray, payment: ByteArray, payerCredential: ByteArray, certificates: ByteArray): KagemushaWalletCallV1 =
        execute(KagemushaWalletOperationInputV1(requestId, 2, first = payment, second = payerCredential, third = certificates))

    /** Receive the exact Payment and payer companion of one carrier (`IrohaPeerWalletPaymentV1`). Native
     * checks the companion against the Payment, completes the durably issued open Request from it and
     * performs ordinary Receive authentication; the companion is never payment authority.
     */
    fun receiveWithCompanion(requestId: ByteArray, payment: ByteArray, companion: ByteArray): KagemushaWalletCallV1 = task.useCall { receiveWithCompanionAccepted(requestId, payment, companion) }
    private fun receiveWithCompanionAccepted(requestId: ByteArray, payment: ByteArray, companion: ByteArray): KagemushaWalletCallV1 =
        execute(KagemushaWalletOperationInputV1(requestId, 10, first = payment, second = companion))

    /** Refresh from exact signed originals; Native derives maps and all effective values. */
    fun refresh(requestId: ByteArray, kind: KagemushaWalletRefreshKindV1, update: ByteArray, certificates: ByteArray): KagemushaWalletCallV1 = task.useCall { refreshAccepted(requestId, kind, update, certificates) }
    private fun refreshAccepted(requestId: ByteArray, kind: KagemushaWalletRefreshKindV1, update: ByteArray, certificates: ByteArray): KagemushaWalletCallV1 =
        execute(KagemushaWalletOperationInputV1(requestId, kind.selector, first = update, second = certificates))

    /** Install a signed blacklist locally; a replacement's list version must increase and
     * reinstalling the held list is idempotent. The wallet then refuses to pay a listed receiver
     * and to request or accept money from a listed payer ([KagemushaWalletExceptionV1.BLOCKED]).
     * Nothing is sent to peers or proved; this is no wallet operation and has no retry identity.
     * COMPLETE carries the held list's 32-byte object digest.
     */
    fun installBlacklist(list: ByteArray, certificates: ByteArray): KagemushaWalletCallV1 = task.useCall { installBlacklistAccepted(list, certificates) }
    private fun installBlacklistAccepted(list: ByteArray, certificates: ByteArray): KagemushaWalletCallV1 =
        execute(KagemushaWalletOperationInputV1(ByteArray(32), 5, first = list, second = certificates))

    /** Authenticate the exact payout, signed charge and canonical beneficiary before confirmation. */
    fun reviewUnload(requestId: ByteArray, amount: KagemushaWalletUInt128V1, quote: ByteArray? = null, certificates: ByteArray? = null,
        chargeBeneficiary: ByteArray? = null, expectation: KagemushaWalletReviewExpectationV1): KagemushaWalletReviewDecisionV1 = task.useCall {
        reviewUnloadAccepted(requestId, amount, quote, certificates, chargeBeneficiary, expectation)
    }
    private fun reviewUnloadAccepted(requestId: ByteArray, amount: KagemushaWalletUInt128V1, quote: ByteArray?, certificates: ByteArray?,
        chargeBeneficiary: ByteArray?, expectation: KagemushaWalletReviewExpectationV1): KagemushaWalletReviewDecisionV1 {
        require((quote == null) == (certificates == null) && (quote == null) == (chargeBeneficiary == null)) {
            "quote, certificates and beneficiary must be supplied together"
        }
        if (quote != null) require(quote.isNotEmpty())
        require(amount.low != 0L || amount.high != 0L)
        require((quote?.size ?: 0) <= 1_024)
        val companion = if (quote == null) byteArrayOf()
            else KagemushaWalletUnloadChargeReviewV1.encode(checkNotNull(certificates), checkNotNull(chargeBeneficiary))
        return review(KagemushaWalletReviewInputV1(requestId, 8, expectation, amount, quote ?: byteArrayOf(), companion),
            KagemushaWalletReviewProjectionV1.Kind.UNLOAD)
    }

    /** Consume the exact reviewed Native operation after fresh hardware confirmation. Native rechecks its whole source. */
    fun executeReviewed(review: KagemushaWalletReviewV1): KagemushaWalletReviewedExecutionV1 = task.useCall { executeReviewedAccepted(review) }
    private fun executeReviewedAccepted(review: KagemushaWalletReviewV1): KagemushaWalletReviewedExecutionV1 {
        val value = handle()
        val token = review.consume(exchange)
        return KagemushaWalletReviewExecutionCleanupV1.run(execute = {
            val reply = KagemushaWalletNativeCallsV1.executeReviewed(value, exchange.token, token)
                ?: throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
            reply.reviewedExecution(exchange, review, token)
        }, discard = { KagemushaWalletNativeCallsV1.discardReview(value, exchange.token, token) }, close = wallet::close)
    }

    /** Enter Retiring under native folded-state checks. */
    fun retire(requestId: ByteArray): KagemushaWalletCallV1 = task.useCall { retireAccepted(requestId) }
    private fun retireAccepted(requestId: ByteArray): KagemushaWalletCallV1 = execute(KagemushaWalletOperationInputV1(requestId, 9))

    private fun execute(input: KagemushaWalletOperationInputV1): KagemushaWalletCallV1 {
        val value = KagemushaWalletNativeCallsV1.execute(handle(), exchange.token, input.requestId(), input.selector,
            input.amount.low, input.amount.high, input.first(), input.second(), input.third())
            ?: throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        if (value.status < 0) throw KagemushaWalletExceptionV1(value.status, value.reason, value.platformCode)
        return value.completion()
    }

    private fun review(input: KagemushaWalletReviewInputV1, expected: KagemushaWalletReviewProjectionV1.Kind): KagemushaWalletReviewDecisionV1 {
        val wallet = handle()
        var reply: KagemushaWalletReviewReplyV1? = null
        try {
            reply = KagemushaWalletNativeCallsV1.review(wallet, exchange.token, input.requestId(), input.selector,
                input.amount.low, input.amount.high, input.expectation.tag, input.expectation.bytes(), input.first(), input.second())
            return (reply ?: throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)).decision(exchange, input.requestId(), expected)
        } catch (failure: KagemushaWalletExceptionV1) {
            if (failure.status == KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT) {
                val token = reply?.cleanupToken()
                try {
                    if (token == null) this.wallet.close()
                    else checked(KagemushaWalletNativeCallsV1.discardReview(wallet, exchange.token, token))
                } catch (cleanup: Throwable) {
                    if(failure !== cleanup)failure.addSuppressed(cleanup)
                    try { this.wallet.close() } catch (retirement: Throwable) { if(failure !== retirement)failure.addSuppressed(retirement) }
                }
            }
            throw failure
        }
    }

    /** Retain exact incoming bytes before verifying their binding to the committed local Send. */
    fun observeCreditedForSend(sendRequestId: ByteArray, original: ByteArray): KagemushaWalletCreditProjectionV1 =
        task.useCall { KagemushaWalletCreditProjectionV1(setup(KagemushaWalletSetupInputV1(56,
            identity = sendRequestId, first = original))).requirePayer() }
    private companion object {
        fun word(bytes: ByteArray) { require(bytes.size == 32) { "identity must be exactly 32 bytes" } }
        fun checked(status: Int) { if (status != 0) throw KagemushaWalletExceptionV1(status) }
    }
}
