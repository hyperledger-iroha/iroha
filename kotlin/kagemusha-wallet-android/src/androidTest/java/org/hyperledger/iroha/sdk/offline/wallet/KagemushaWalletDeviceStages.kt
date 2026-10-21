// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import android.os.SystemClock

/** One live actual capability shared by the existing monetary and carrier command owners. */
internal class KagemushaProbeReviewAttempt(
    val review: KagemushaWalletReviewV1, val origin: KagemushaWalletExchangeV1,
    val wallet: KagemushaWalletV1, val sequence: Int, val fence: String, val continuation: Boolean,
) {
    val requestId = review.requestId()
    val phase = KagemushaProbeReviewPhase()
}
internal class KagemushaProbeReviewHolder {
    var current: KagemushaProbeReviewAttempt? = null
        private set
    fun retain(value: KagemushaProbeReviewAttempt) { check(current == null); current = value }
    fun clear(value: KagemushaProbeReviewAttempt) { check(current === value); current = null }
    fun released() { current?.phase?.finish(); current = null }
    fun endRequested(origin: KagemushaWalletExchangeV1): KagemushaProbeReviewAttempt? {
        val selected = current ?: return null
        check(selected.origin === origin)
        selected.phase.endRequested(); selected.phase.finish(); current = null
        return selected
    }
}

/** Worker-only real SDK monetary lifecycle. This driver neither signs ledger transactions nor owns keys. */
internal class KagemushaWalletDeviceStages(
    private val config: EnrollmentProbeConfiguration,
    private val walletOwner: () -> KagemushaWalletV1,
    private val exchangeOwner: () -> KagemushaWalletExchangeV1,
    private val output: (String, ByteArray, Int) -> Unit,
    private val requireLive: () -> Unit,
    private val reviews: KagemushaProbeReviewHolder,
) {
    private class PendingTime(val challenge: KagemushaWalletTimeExchangeV1, val origin: KagemushaWalletExchangeV1)
    private var timeExchange: PendingTime? = null
    /** Call only after the owning wallet acknowledged close. Native owns the actual lifetime. */
    fun released() { reviews.released(); timeExchange = null }
    private fun event(name: String, sequence: Int, vararg fields: Pair<String, String>) =
        output("$name.json", probeJson(*fields), sequence)
    private fun hex(bytes: ByteArray) = EnrollmentProbeFiles.hex(bytes)
    private fun unsigned(value: Long) = KagemushaMonetaryProbeManifest.unsigned(value)
    private fun scalar(name: String, value: KagemushaWalletUInt128V1) = listOf(
        "${name}Low" to unsigned(value.low), "${name}High" to unsigned(value.high))
    private fun inputs(command: EnrollmentProbeCommand, sequence: Int): Map<String, ByteArray> {
        KagemushaMonetaryProbeManifest.validate(command.action, command.pins.keys)
        return command.pins.toSortedMap().mapValues { (name, pin) ->
            probeRequire(pin.path.startsWith("inbox/${config.invocation}/"))
            config.files.pinned(pin, KagemushaMonetaryProbeManifest.maximum(name)).also { bytes ->
                probeRequire(bytes.isNotEmpty())
                config.files.publish("private/${config.invocation}/${sequence.toString().padStart(4, '0')}-$name", bytes)
            }
        }
    }
    /** Immutable audit guard only; Native remains the sole recovery authority. */
    private fun bind(kind: String, id: ByteArray, inputs: Map<String, ByteArray>) {
        config.files.mkdir("private/intents")
        config.files.publish("private/intents/$kind-${EnrollmentProbeFiles.digest(id)}",
            KagemushaMonetaryProbeManifest.binding(kind, inputs))
    }
    private fun executedPath(id: ByteArray) = "private/intents/executed-${EnrollmentProbeFiles.digest(id)}"
    private fun call(value: KagemushaWalletCallV1, returned: Long, sequence: Int): String {
        val bytes = value.bytes()
        output("native-call.bin", bytes, sequence)
        event("native-call", sequence, "status" to value.status.toString(),
            "sequenceLow" to unsigned(value.sequenceLow), "sequenceHigh" to unsigned(value.sequenceHigh),
            "detail" to value.detail.toString(), "nativeReturnedTicks" to returned.toString(),
            "originalSHA256" to EnrollmentProbeFiles.digest(bytes))
        // Pending, Preparing, Unknown, Archived and delivery-data loss are preserved verbatim.
        return "native-call-status-${value.status}"
    }
    private fun released(value: KagemushaWalletReleasedOutputV1, observed: Long, sequence: Int): String {
        output("released-original.norito", value.original(), sequence)
        output("operation-id.bin", value.operationId(), sequence)
        value.peerKind?.let { kind ->
            val original = value.peerOriginal() ?: throw EnrollmentProbeFailure("native-projection")
            output("peer-original.norito", original, sequence)
            output("peer-envelope.norito", walletOwner().envelope(kind, original), sequence)
        }
        event("released", sequence, *(listOf("kind" to value.kind.name,
            "operationID" to hex(value.operationId()), "durableCompletionObservedTicks" to observed.toString(),
            "originalSHA256" to EnrollmentProbeFiles.digest(value.original()),
            "carrierDeliveryClaimed" to "false") + scalar("sequence", value.sequence)).toTypedArray())
        return when (value.kind) {
            KagemushaWalletReleasedOutputV1.Kind.LOAD -> "native-complete-load"
            KagemushaWalletReleasedOutputV1.Kind.SEND -> "native-complete-send"
            KagemushaWalletReleasedOutputV1.Kind.RECEIVE -> "native-complete-receive"
            else -> "native-released-kind-${value.kind.name}"
        }
    }
    private fun completion(value: KagemushaWalletCallV1, returned: Long, requestId: ByteArray, sequence: Int): String {
        val status = call(value, returned, sequence)
        if (value.status != KagemushaWalletCallV1.COMPLETE) return status
        val original = walletOwner().releasedOutputForRequest(requestId)
        // Observations after recovery do not reconstruct the unknown historical commit time.
        event("durable-call-return", sequence, "nativeReturnedTicks" to returned.toString(),
            "requestID" to hex(requestId), "operationID" to hex(original.operationId()))
        return released(original, SystemClock.elapsedRealtimeNanos(), sequence)
    }
    private fun activationState(value: KagemushaWalletActivationFinalityV1, returned: Long, sequence: Int): String {
        val row = mutableListOf("nativeReturnedTicks" to returned.toString(),
            "confirmed" to (value.confirmation != null).toString(), "rejected" to value.rejected.toString())
        value.verifiedHeightBits?.let { row.add("height" to unsigned(it)) }
        value.blockHash()?.let { row.add("blockHash" to hex(it)) }
        event("activation-state", sequence, *row.toTypedArray())
        return if (value.confirmation != null) "native-activation-confirmed"
            else if (value.rejected) "native-activation-rejected" else "native-activation-unconfirmed"
    }
    private fun ledgerState(value: KagemushaWalletLedgerProgressV1?, returned: Long, sequence: Int) {
        val row = mutableListOf("nativeReturnedTicks" to returned.toString(), "present" to (value != null).toString())
        value?.let { row.add("height" to unsigned(it.heightBits)); row.add("blockHash" to hex(it.blockHash())) }
        event("ledger-progress", sequence, *row.toTypedArray())
    }
    private fun snapshot(sequence: Int): String {
        val value = walletOwner().snapshot()
        val observed = SystemClock.elapsedRealtimeNanos()
        val row = mutableListOf("observedTicks" to observed.toString(), "head" to hex(value.head()),
            "credentialDigest" to hex(value.credentialDigest()), "headIsFolded" to value.headIsFolded.toString(),
            "lifecycle" to value.lifecycle.name, "operationPermissionClaimed" to "false")
        row.addAll(scalar("sequence", value.sequence)); row.addAll(scalar("ownedBalance", value.ownedBalance))
        row.addAll(scalar("knownBurnedTotal", value.knownBurnedTotal)); row.addAll(scalar("backlog", value.foldBacklog))
        value.foldedBalance?.let { row.addAll(scalar("foldedBalance", it)); row.add("currentHeadFoldReadyObservedTicks" to observed.toString()) }
        event("snapshot", sequence, *row.toTypedArray())
        return if (value.headIsFolded) "native-current-head-folded" else "native-current-head-not-folded"
    }
    fun execute(command: EnrollmentProbeCommand, sequence: Int): String {
        // The mailbox is worker-synchronous. Admission precedes every SDK worker and UI wait.
        val fresh = command.action in setOf("bootstrap", "activation", "retain-activation",
            "activation-finality", "confirm-activation", "prepare-load", "load", "ledger-finality",
            "epoch-boundary", "request", "review-send", "review-unload", "continue-review-send", "continue-review-unload", "receive", "observe-credited",
            "unload-finality", "confirm-unload", "refresh-time", "begin-time")
        val origin = if (fresh) exchangeOwner() else null
        val admitted = origin?.admit()
        try { return executeAccepted(command, sequence, origin, admitted) }
        finally { admitted?.finish() }
    }
    private fun executeAccepted(command: EnrollmentProbeCommand, sequence: Int,
        origin: KagemushaWalletExchangeV1?, admitted: KagemushaWalletExchangeCallV1?): String {
        if (command.action !in setOf("discard-review", "cancel-time", "decide-review", "retry-reviewed")) requireLive()
        fun accepted() = admitted ?: throw EnrollmentProbeFailure("exchange-required")
        val values = inputs(command, sequence)
        fun input(name: String) = values[name] ?: throw EnrollmentProbeFailure("input")
        fun id(name: String = "request-id") = KagemushaMonetaryProbeManifest.word(input(name))
        fun family(name: String) = values + ("family" to name.toByteArray(Charsets.UTF_8))
        val wallet = walletOwner()
        return when (command.action) {
            "bootstrap" -> {
                val value = accepted().bootstrap(); call(value, SystemClock.elapsedRealtimeNanos(), sequence)
            }
            "activation" -> {
                val original = accepted().activation(); val returned = SystemClock.elapsedRealtimeNanos()
                output("activation.norito", original, sequence)
                output("ledger-instruction.norito", wallet.ledgerInstruction(KagemushaWalletLedgerTransportV1.ACTIVATE, original), sequence)
                event("activation-export", sequence, "nativeReturnedTicks" to returned.toString(), "confirmed" to "false")
                "native-activation-exported-not-submitted"
            }
            "retain-activation", "activation-status", "activation-finality", "confirm-activation" -> {
                val wire = input("signed-wire")
                if (command.action == "confirm-activation") {
                    val value = accepted().confirmLedgerActivation(wire); val returned = SystemClock.elapsedRealtimeNanos()
                    event("activation-confirmation", sequence, "nativeReturnedTicks" to returned.toString(),
                        "height" to unsigned(value.heightBits), "blockHash" to hex(value.blockHash()))
                    "native-activation-confirmed"
                } else {
                    val value = when (command.action) {
                        "retain-activation" -> accepted().retainActivationAttempt(wire)
                        "activation-finality" -> accepted().ingestActivationFinality(wire, input("finality"))
                        else -> wallet.activationFinalityProgress(wire)
                    }
                    val returned = SystemClock.elapsedRealtimeNanos()
                    if (command.action == "retain-activation") {
                        activationRetentionReceipt(wire, value, returned, config.invocation, sequence)?.let {
                            output("activation-retained.json", it, sequence)
                        }
                    }
                    activationState(value, returned, sequence)
                }
            }
            "prepare-load" -> {
                val identity = id(); val amount = KagemushaMonetaryProbeManifest.amount(input("amount"))
                bind("load-plan", identity, values)
                val original = accepted().prepareLedgerLoad(identity, amount); val returned = SystemClock.elapsedRealtimeNanos()
                output("ledger-load-instruction.norito", original, sequence)
                event("prepared-load", sequence, "nativeReturnedTicks" to returned.toString(),
                    "requestID" to hex(identity), "settlementClaimed" to "false")
                "native-load-plan-retained"
            }
            "prepared-load" -> {
                val plan = wallet.preparedLedgerLoad(id()); val returned = SystemClock.elapsedRealtimeNanos()
                if (plan == null) "native-load-plan-absent" else {
                    output("ledger-load-instruction.norito", plan.instructionOriginal(), sequence)
                    event("prepared-load", sequence, *(listOf("nativeReturnedTicks" to returned.toString(),
                        "requestID" to hex(plan.requestId())) + scalar("ordinal", plan.ordinal) +
                        scalar("amount", plan.amount) + scalar("onlineCharge", plan.onlineCharge)).toTypedArray())
                    "native-load-plan-retained"
                }
            }
            "load" -> {
                val identity = id(); bind("operation", identity, family("load"))
                val value = accepted().load(identity, input("receipt"), input("finality"))
                completion(value, SystemClock.elapsedRealtimeNanos(), identity, sequence)
            }
            "ledger-finality", "ledger-status" -> {
                val value = if (command.action == "ledger-finality") accepted().ingestLedgerFinality(input("finality")) else wallet.ledgerProgress()
                ledgerState(value, SystemClock.elapsedRealtimeNanos(), sequence); "native-ledger-prefix-observed"
            }
            "epoch-boundary", "epoch-status" -> {
                val value = if (command.action == "epoch-boundary") accepted().ingestEpochBoundary(
                    KagemushaMonetaryProbeManifest.limb(input("epoch")), input("boundary")) else wallet.epochProgress()
                event("epoch", sequence, "nativeReturnedTicks" to SystemClock.elapsedRealtimeNanos().toString(),
                    "epoch" to value.epoch.toString(), "firstHeight" to value.firstHeight.toString(),
                    "boundaryHeight" to value.boundaryHeight.toString())
                "native-epoch-prefix-observed"
            }
            "request" -> {
                val identity = id("setup-id"); bind("setup", identity, family(command.action))
                val original = accepted().request(identity, KagemushaMonetaryProbeManifest.amount(input("amount")),
                    values["fee-schedule"], values["fee-certificate"])
                val returned = SystemClock.elapsedRealtimeNanos()
                val kind = KagemushaWalletTransportKindV1.REQUEST
                output("peer-original.norito", original, sequence)
                output("peer-envelope.norito", wallet.envelope(kind, original), sequence)
                event("setup-export", sequence, "nativeReturnedTicks" to returned.toString(),
                    "kind" to kind.name, "setupID" to hex(identity), "carrierDeliveryClaimed" to "false")
                "native-${command.action}-exported-carrier-unobserved"
            }
            "review-send", "review-unload", "continue-review-send", "continue-review-unload" -> {
                probeRequire(reviews.current == null, "lifecycle")
                val continuation = command.action.startsWith("continue-")
                val actionFamily = command.action.removePrefix("continue-")
                val identity = id()
                val binding = "private/intents/operation-${EnrollmentProbeFiles.digest(identity)}"
                if (continuation) probeRequire(config.files.exists(binding), "recovery-required")
                bind("operation", identity, family(actionFamily))
                val root = executedPath(identity)
                val expectation = if (continuation) KagemushaWalletReviewExpectationV1.OriginalUnstarted(
                    config.files.read(root, KagemushaWalletReviewProjectionV1.MAXIMUM_BYTES)) else {
                    probeRequire(!config.files.exists(root), "recovery-required")
                    KagemushaWalletReviewExpectationV1.Fresh
                }
                val decision = if (actionFamily == "review-send") accepted().reviewSend(identity, input("request"), input("destination"), expectation)
                    else accepted().reviewUnload(identity, KagemushaMonetaryProbeManifest.amount(input("amount")),
                        values["quote"], values["certificates"], values["beneficiary"], expectation)
                val returned = SystemClock.elapsedRealtimeNanos()
                when (decision) {
                    is KagemushaWalletReviewDecisionV1.Existing -> existing(decision.status, returned, sequence)
                    is KagemushaWalletReviewDecisionV1.Waiting -> {
                        event("review-waiting", sequence, "wait" to decision.wait.name, "requestID" to hex(identity))
                        "native-review-waiting-${decision.wait.name}"
                    }
                    is KagemushaWalletReviewDecisionV1.Ready -> {
                        val review = decision.review
                        val fence = if (continuation) "$root-attempt-${config.invocation}-$sequence" else root
                        val attempt = KagemushaProbeReviewAttempt(review, origin ?: throw EnrollmentProbeFailure("exchange-required"),
                            wallet, sequence, fence, continuation)
                        reviews.retain(attempt)
                        try {
                            if (continuation) config.files.publish("$fence-binding", KagemushaMonetaryProbeManifest.binding("review-attempt",
                                mapOf("request-id" to identity, "root-projection" to expectation.bytes(), "projection" to review.projection.bytes())), exclusive = true)
                            val p = review.projection
                            output("native-review.bin", p.bytes(), sequence)
                            event("native-review", sequence, *(listOf("nativeReturnedTicks" to returned.toString(),
                                "reviewSHA256" to EnrollmentProbeFiles.digest(p.bytes()), "kind" to p.kind.name,
                                "requestID" to hex(identity), "head" to hex(p.currentHead()),
                                "sourceStateCommitment" to hex(p.sourceStateCommitment()),
                                "destinationAccountDigest" to hex(p.destinationAccountDigest()), "continuation" to continuation.toString(),
                                "onwardSendReviewReadyObserved" to (p.kind == KagemushaWalletReviewProjectionV1.Kind.SEND).toString()) +
                                scalar("amount", p.amount) + scalar("fee", p.fee) + scalar("grossDebit", p.grossDebit) +
                                scalar("netDestinationAmount", p.netDestinationAmount)).toTypedArray())
                        } catch (failure: Throwable) { discardAfterFailure(attempt, failure); throw failure }
                        "native-review-awaiting-explicit-decision"
                    }
                }
            }
            "decide-review" -> decideReview(input("decision"), sequence)
            "retry-reviewed" -> retryReviewed(sequence)
            "discard-review" -> {
                val pending = reviews.current ?: throw EnrollmentProbeFailure("lifecycle")
                reviews.clear(pending); pending.phase.finish()
                pending.origin.discardReview(pending.review)
                "native-review-cancelled"
            }
            "payment-companion" -> {
                val companion = wallet.paymentCompanion(input("payment")); val returned = SystemClock.elapsedRealtimeNanos()
                output("payer-companion.norito", companion, sequence)
                event("payment-companion", sequence, "nativeReturnedTicks" to returned.toString(),
                    "carrierDeliveryClaimed" to "false", "monetaryAdmissionClaimed" to "false")
                "native-payment-companion-exported-carrier-unobserved"
            }
            "receive" -> {
                val identity = id(); bind("operation", identity, family("receive"))
                val value = accepted().receiveWithCompanion(identity, input("payment"), input("companion"))
                completion(value, SystemClock.elapsedRealtimeNanos(), identity, sequence)
            }
            "credit" -> {
                val value = wallet.receivedCredit(id()); val returned = SystemClock.elapsedRealtimeNanos()
                event("credit", sequence, *(listOf("nativeReturnedTicks" to returned.toString(),
                    "evidence" to value.evidence.name, "archive" to value.archive.name, "corePending" to value.corePending.toString(),
                    "creditID" to hex(value.creditId()), "paymentDigest" to hex(value.paymentDigest())) + scalar("amount", value.amount)).toTypedArray())
                value.creditedOriginal()?.let { original ->
                    output("peer-original.norito", original, sequence)
                    output("peer-envelope.norito", wallet.envelope(KagemushaWalletTransportKindV1.CREDITED, original), sequence)
                }
                "native-credit-evidence-${value.evidence.name}"
            }
            "observe-credited" -> {
                val credited = input("credited")
                output("verified-credited-original.norito", credited, sequence)
                val value = accepted().observeCreditedForSend(id(), credited)
                val returned = SystemClock.elapsedRealtimeNanos()
                event("verified-delivery", sequence, *(listOf(
                    "nativeReturnedTicks" to returned.toString(), "creditedSHA256" to EnrollmentProbeFiles.digest(credited),
                    "evidence" to value.evidence.name, "archive" to value.archive.name,
                    "corePending" to value.corePending.toString(), "creditID" to hex(value.creditId()),
                    "paymentDigest" to hex(value.paymentDigest()), "carrierDeliveryClaimed" to "false") +
                    scalar("amount", value.amount)).toTypedArray())
                "native-payer-delivery-evidence-${value.evidence.name}"
            }
            "credited-status" -> {
                val value = wallet.creditedStatusForSend(id(), input("credited"))
                call(value, SystemClock.elapsedRealtimeNanos(), sequence)
            }
            "resume-deferred-receipt" -> {
                val value = wallet.verifyNextDeferredCredited()
                val returned = SystemClock.elapsedRealtimeNanos()
                event("deferred-receipt", sequence, "nativeReturnedTicks" to returned.toString(),
                    "evidence" to (value?.evidence?.name ?: "none"),
                    "archive" to (value?.archive?.name ?: "none"),
                    "corePending" to (value?.corePending?.toString() ?: "unknown"),
                    "carrierDeliveryClaimed" to "false")
                if (value == null) "native-deferred-receipt-no-work" else "native-payer-delivery-evidence-${value.evidence.name}"
            }
            "drain-deferred-archive" -> {
                val value = wallet.drainDeferredArchive()
                val returned = SystemClock.elapsedRealtimeNanos()
                when (value) {
                    KagemushaWalletDeferredArchiveDrainV1.NoWork -> "native-deferred-archive-no-work"
                    is KagemushaWalletDeferredArchiveDrainV1.Blocked -> "native-deferred-archive-blocked-${value.wait.name}"
                    is KagemushaWalletDeferredArchiveDrainV1.Reconciled -> call(value.completion, returned, sequence)
                    is KagemushaWalletDeferredArchiveDrainV1.Executed -> call(value.completion, returned, sequence)
                }
            }
            "fold-once" -> {
                val value = wallet.foldOnce(); call(value, SystemClock.elapsedRealtimeNanos(), sequence); snapshot(sequence)
            }
            "snapshot" -> snapshot(sequence)
            "fold-status" -> {
                val value = wallet.foldStatus(); val returned = SystemClock.elapsedRealtimeNanos()
                val fields = mutableListOf("nativeReturnedTicks" to returned.toString(),
                    "phase" to value.phase.name, "eligible" to value.eligible.toString())
                value.observedBacklog?.let { fields.addAll(scalar("observedBacklog", it)) }
                event("fold-status", sequence, *fields.toTypedArray()); "native-fold-worker-observed"
            }
            "request-status" -> {
                val identity = id(); val value = wallet.requestStatus(identity)
                completion(value, SystemClock.elapsedRealtimeNanos(), identity, sequence)
            }
            "retry" -> {
                val operation = id("operation-id"); val value = wallet.retry(operation)
                val status = call(value, SystemClock.elapsedRealtimeNanos(), sequence)
                if (value.status == KagemushaWalletCallV1.COMPLETE) {
                    val original = wallet.releasedOutputForOperation(operation)
                    released(original, SystemClock.elapsedRealtimeNanos(), sequence)
                } else status
            }
            "resume" -> {
                val value = wallet.resume(); call(value, SystemClock.elapsedRealtimeNanos(), sequence)
            }
            "released-request" -> {
                val value = wallet.releasedOutputForRequest(id()); released(value, SystemClock.elapsedRealtimeNanos(), sequence)
            }
            "released-operation" -> {
                val value = wallet.releasedOutputForOperation(id("operation-id")); released(value, SystemClock.elapsedRealtimeNanos(), sequence)
            }
            "unload-claim" -> {
                val original = wallet.unloadClaimTransport(id(), values["beneficiary"]); val returned = SystemClock.elapsedRealtimeNanos()
                output("unload-claim.norito", original, sequence)
                output("ledger-instruction.norito", wallet.ledgerInstruction(KagemushaWalletLedgerTransportV1.UNLOAD, original), sequence)
                event("unload-claim", sequence, "nativeReturnedTicks" to returned.toString(), "settled" to "false")
                "native-unload-claim-exported-not-settled"
            }
            "unload-status" -> {
                val value = wallet.unloadFinalityProgress(id("transaction-hash"), input("claim")); val returned = SystemClock.elapsedRealtimeNanos()
                val fields = mutableListOf("nativeReturnedTicks" to returned.toString(), "confirmed" to (value.confirmation != null).toString())
                value.verifiedHeightBits?.let { fields.add("height" to unsigned(it)) }; value.blockHash()?.let { fields.add("blockHash" to hex(it)) }
                event("unload-status", sequence, *fields.toTypedArray())
                if (value.confirmation == null) "native-unload-unconfirmed" else "native-unload-confirmed"
            }
            "unload-finality" -> {
                val value = accepted().ingestUnloadFinality(id("transaction-hash"), input("claim"), input("finality"))
                ledgerState(value, SystemClock.elapsedRealtimeNanos(), sequence); "native-unload-prefix-retained-not-confirmed"
            }
            "confirm-unload" -> {
                val value = accepted().confirmLedgerUnload(id("transaction-hash"), input("claim")); val returned = SystemClock.elapsedRealtimeNanos()
                event("unload-confirmation", sequence, "nativeReturnedTicks" to returned.toString(),
                    "height" to unsigned(value.heightBits), "blockHash" to hex(value.blockHash()))
                "native-unload-confirmed"
            }
            "decode-request", "decode-payment", "decode-credited" -> {
                val kind = when (command.action) {
                    "decode-request" -> KagemushaWalletTransportKindV1.REQUEST
                    "decode-payment" -> KagemushaWalletTransportKindV1.PAYMENT
                    else -> KagemushaWalletTransportKindV1.CREDITED
                }
                val original = wallet.original(kind, input("envelope")); val returned = SystemClock.elapsedRealtimeNanos()
                output("peer-original.norito", original, sequence)
                event("peer-decoded", sequence, "nativeReturnedTicks" to returned.toString(), "kind" to kind.name,
                    "carrierDeliveryClaimed" to "false", "monetaryAdmissionClaimed" to "false")
                "native-peer-envelope-decoded-not-admitted"
            }
            "refresh-time" -> {
                val identity = id(); bind("operation", identity, family("refresh-time"))
                val value = accepted().refresh(identity, KagemushaWalletRefreshKindV1.TIME_ANCHOR, input("anchor"), input("certificates"))
                completion(value, SystemClock.elapsedRealtimeNanos(), identity, sequence)
            }
            "begin-time" -> {
                probeRequire(timeExchange == null, "lifecycle")
                val exchange = accepted().beginTimeExchange(); val returned = SystemClock.elapsedRealtimeNanos()
                timeExchange = PendingTime(exchange, origin ?: throw EnrollmentProbeFailure("exchange-required")); output("time-nonce.bin", exchange.nonce(), sequence)
                event("time-exchange", sequence, "nativeReturnedTicks" to returned.toString())
                "native-time-exchange-awaiting-issuer"
            }
            "finish-time" -> {
                val exchange = timeExchange ?: throw EnrollmentProbeFailure("lifecycle")
                // All bounded input reads precede consumption; never reconstruct the Native token.
                val anchor = input("anchor"); val certificate = input("certificate")
                exchange.origin.admit().use { accepted ->
                    timeExchange = null
                    accepted.finishTimeExchange(exchange.challenge, anchor, certificate)
                }
                event("time-retained", sequence, "nativeReturnedTicks" to SystemClock.elapsedRealtimeNanos().toString())
                "native-time-response-retained-refresh-required"
            }
            "cancel-time" -> {
                val exchange = timeExchange ?: throw EnrollmentProbeFailure("lifecycle")
                timeExchange = null; exchange.origin.cancelTimeExchange(exchange.challenge); "native-time-exchange-cancelled"
            }
            else -> throw EnrollmentProbeFailure("input")
        }
    }
    private fun existing(value: KagemushaWalletBoundRequestStatusV1, returned: Long, sequence: Int): String {
        val status = when (value) {
            KagemushaWalletBoundRequestStatusV1.Preparing -> KagemushaWalletCallV1.PREPARING
            is KagemushaWalletBoundRequestStatusV1.Outcome -> { call(value.completion, returned, sequence); value.completion.status }
        }
        event("existing-request", sequence, "status" to status.toString(), "newExecutionClaimed" to "false", "latencyUsable" to "false")
        return "native-existing-request-status-$status"
    }
    private fun requireAttempt(pending: KagemushaProbeReviewAttempt) {
        requireLive()
        probeRequire(reviews.current === pending && walletOwner() === pending.wallet && exchangeOwner() === pending.origin, "lifecycle")
    }
    private fun discardAfterFailure(pending: KagemushaProbeReviewAttempt, failure: Throwable) {
        if (reviews.current === pending) reviews.clear(pending)
        pending.phase.finish()
        try { pending.origin.discardReview(pending.review) }
        catch (cleanup: Throwable) {
            if (cleanup !is KagemushaWalletExceptionV1 || cleanup.status != -2) if (cleanup !== failure) failure.addSuppressed(cleanup)
        }
    }
    private fun executeReview(pending: KagemushaProbeReviewAttempt, accepted: KagemushaWalletExchangeCallV1,
        sequence: Int, retry: Boolean): String {
        requireAttempt(pending)
        event("execute-reviewed-start", sequence, "ticks" to SystemClock.elapsedRealtimeNanos().toString(),
            "requestID" to hex(pending.requestId), "reviewSHA256" to EnrollmentProbeFiles.digest(pending.review.projection.bytes()),
            "liveRetry" to retry.toString(), "continuation" to pending.continuation.toString(),
            "originalLatencyUsable" to (!retry && !pending.continuation).toString())
        return try {
            when (val result = accepted.executeReviewed(pending.review)) {
                is KagemushaWalletReviewedExecutionV1.RetainedReview -> {
                    probeRequire(result.review === pending.review, "native-projection")
                    pending.phase.retained()
                    event("review-retained", sequence, "wait" to result.wait.name, "reviewSequence" to pending.sequence.toString(),
                        "dispatchFenceRetained" to "true", "latencyUsable" to "false")
                    "native-review-retained-${result.wait.name}"
                }
                is KagemushaWalletReviewedExecutionV1.Existing -> {
                    reviews.clear(pending); pending.phase.finish()
                    existing(result.status, SystemClock.elapsedRealtimeNanos(), sequence)
                }
                is KagemushaWalletReviewedExecutionV1.Executed -> {
                    reviews.clear(pending); pending.phase.finish()
                    completion(result.completion, SystemClock.elapsedRealtimeNanos(), pending.requestId, sequence)
                }
            }
        } catch (failure: Throwable) { discardAfterFailure(pending, failure); throw failure }
    }
    private fun retryReviewed(sequence: Int): String {
        val pending = reviews.current ?: throw EnrollmentProbeFailure("lifecycle")
        probeRequire(pending.phase.state == KagemushaProbeReviewPhase.State.APPROVED_LIVE_RETAINED, "lifecycle")
        try {
            pending.origin.admit().use { accepted ->
                requireAttempt(pending)
                pending.phase.retry {
                    probeRequire(config.files.read(pending.fence, KagemushaWalletReviewProjectionV1.MAXIMUM_BYTES)
                        .contentEquals(pending.review.projection.bytes()), "recovery-required")
                }
                return executeReview(pending, accepted, sequence, retry = true)
            }
        } catch (failure: Throwable) { discardAfterFailure(pending, failure); throw failure }
    }
    private fun decideReview(bytes: ByteArray, sequence: Int): String {
        val pending = reviews.current ?: throw EnrollmentProbeFailure("lifecycle")
        probeRequire(pending.phase.state == KagemushaProbeReviewPhase.State.AWAITING_CONFIRMATION, "lifecycle")
        val projection = pending.review.projection
        val decision = KagemushaMonetaryProbeManifest.Decision(bytes, config.invocation, pending.sequence, projection.bytes())
        if (!decision.confirmed) {
            reviews.clear(pending); pending.phase.finish(); pending.origin.discardReview(pending.review)
            event("review-cancelled", sequence, "nativeReturnedTicks" to SystemClock.elapsedRealtimeNanos().toString(),
                "reviewSequence" to pending.sequence.toString())
            return "native-review-cancelled"
        }
        pending.origin.admit().use { accepted ->
            requireAttempt(pending)
            probeRequire(!config.files.exists(pending.fence), "recovery-required")
            pending.phase.beginConfirmation()
            val digest = EnrollmentProbeFiles.digest(projection.bytes())
            val authenticated = try {
                event("confirmation-start", sequence, "ticks" to SystemClock.elapsedRealtimeNanos().toString(),
                    "reviewSequence" to pending.sequence.toString(), "reviewSHA256" to digest)
                KagemushaProbeConfirmation.authenticate(projection).also { requireAttempt(pending) }
            } catch (failure: Throwable) { pending.phase.confirmationFailed(); throw failure }
            try {
                event("confirmation", sequence, "deviceOwnerAuthenticatedTicks" to authenticated.ticks.toString(),
                    "authenticationType" to authenticated.authenticationType.toString(), "promptStartedTicks" to authenticated.promptStartedTicks.toString(),
                    "reviewSequence" to pending.sequence.toString(), "reviewSHA256" to digest, "requestID" to hex(pending.requestId),
                    "confirmationSurface" to "external-exact-review-plus-android-device-owner-authentication")
                pending.phase.publishFence { config.files.publish(pending.fence, projection.bytes(), exclusive = true) }
                return executeReview(pending, accepted, sequence, retry = false)
            } catch (failure: Throwable) { discardAfterFailure(pending, failure); throw failure }
        }
    }
}
