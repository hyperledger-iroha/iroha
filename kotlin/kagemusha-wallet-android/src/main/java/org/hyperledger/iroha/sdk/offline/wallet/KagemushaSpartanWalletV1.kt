// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import org.hyperledger.iroha.sdk.offline.IrohaPeerContentEncodingV1
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcV1
import org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile
import org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadKind
import org.hyperledger.iroha.sdk.offline.IrohaPeerWireMessageV1
import org.hyperledger.iroha.sdk.offline.IrohaPeerSpartanOpenRequestV3
import java.io.Closeable
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.concurrent.atomic.AtomicLong

/** Exact future-profile admission DATA; only Native grants authority. */
class KagemushaSpartanAccountOriginalsV1(credential: ByteArray, enrollmentCertificate: ByteArray,
    account: ByteArray, assetScope: ByteArray) {
    private val originals: KagemushaWalletOpenOriginalsV1
    init {
        require(credential.size == 596 && enrollmentCertificate.size == 172)
        originals = KagemushaWalletOpenOriginalsV1(credential, enrollmentCertificate, account, assetScope)
    }
    internal fun frames(): List<ByteArray> = originals.frames()
}

/** Experimental proof backend, using the existing exclusive hardware custody owner. */
class KagemushaSpartanWalletV1 internal constructor(
    private val owner: KagemushaWalletV1,
    parameterId: ByteArray,
    slotId: ByteArray,
    private val platformOwner: Any,
    /** True when Native admits only exact retained output reads after delivery-data loss. */
    val retainedOutputsOnly: Boolean,
) : Closeable {
    private val parameters = parameterId.copyOf()
    private val slot = slotId.copyOf()
    fun parameterId(): ByteArray = parameters.copyOf()
    fun slotId(): ByteArray = slot.copyOf()

    /** Issue or retrieve the receiver's exact signed Request for one offline exchange. */
    @JvmOverloads fun unloadSettlement(operationId: ByteArray, signedTransaction: ByteArray, finality: ByteArray = byteArrayOf()): KagemushaSpartanUnloadSettlementV1 {
        require(!retainedOutputsOnly && operationId.size == 32 && signedTransaction.size in 1..65536 && finality.size <= 1_327_104)
        val handle = owner.exchangeHandle()
        val op = operationId.copyOf()
        val signed = signedTransaction.copyOf()
        val proof = finality.copyOf()
        val reply = call { NativeSpartanBridge.unloadSettlement(handle, slot.copyOf(), op, signed, proof) }
        return KagemushaSpartanUnloadSettlementV1(reply)
    }

    /** Native's durable activation decision for exactly these account-signed bytes. */
    @JvmOverloads fun activationSettlement(signedTransaction: ByteArray, finality: ByteArray = byteArrayOf()): KagemushaSpartanActivationSettlementV1 {
        require(!retainedOutputsOnly && signedTransaction.size in 1..65_536 && finality.size <= 1_327_104)
        val signed = signedTransaction.copyOf()
        val proof = finality.copyOf()
        return KagemushaSpartanActivationSettlementV1(call {
            NativeSpartanBridge.activationSettlement(owner.exchangeHandle(), slot.copyOf(), signed, proof)
        })
    }

    fun epochProgress(): KagemushaWalletEpochProgressV1 = exchangeEpoch(0, byteArrayOf())

    fun ingestEpochBoundary(expectedEpoch: java.math.BigInteger, original: ByteArray): KagemushaWalletEpochProgressV1 {
        require(expectedEpoch.signum() >= 0 && expectedEpoch.bitLength() <= 64 && original.size in 1..262144)
        return exchangeEpoch(expectedEpoch.toLong(), original.copyOf())
    }

    private fun exchangeEpoch(expected: Long, original: ByteArray): KagemushaWalletEpochProgressV1 {
        require(!retainedOutputsOnly)
        val handle = owner.exchangeHandle()
        val reply = call { NativeSpartanBridge.epoch(handle, slot.copyOf(), expected, original) }
        reply.requireShape(103, 24)
        val bytes = reply.bytes()
        return KagemushaWalletEpochProgressV1(settlementWord(bytes, 0), settlementWord(bytes, 8), settlementWord(bytes, 16))
    }

    @JvmOverloads fun issueOpenRequest(requestId: ByteArray, amount: KagemushaWalletUInt128V1,
        feeSchedule: ByteArray = byteArrayOf(), feeCertificate: ByteArray = byteArrayOf()): IrohaPeerWireMessageV1 {
        val id = requestId.copyOf()
        require(!retainedOutputsOnly && id.size == 32 && id.any { it != 0.toByte() })
        require(amount.low != 0L || amount.high != 0L)
        require((feeSchedule.isEmpty() && feeCertificate.isEmpty()) ||
            (feeSchedule.size == 255 && feeCertificate.size == 172))
        val schedule = feeSchedule.copyOf()
        val certificate = feeCertificate.copyOf()
        val units = ByteBuffer.allocate(16).order(ByteOrder.LITTLE_ENDIAN).putLong(amount.low).putLong(amount.high).array()
        val reply = call { NativeSpartanBridge.issueOpenRequest(owner.exchangeHandle(), slot.copyOf(), id, units, schedule, certificate) }
        return reply.issuedOpenRequest(units, schedule)
    }

    /** Verify and retain a signed regulatory blacklist for this wallet, returning its digest. */
    fun installBlacklist(original: ByteArray, certificates: ByteArray): ByteArray {
        require(original.size in 1..2_228_736 && certificates.size in 1..10_034)
        val handle = owner.exchangeHandle()
        val list = original.copyOf()
        val certs = certificates.copyOf()
        val reply = call { NativeSpartanBridge.installBlacklist(handle, slot.copyOf(), list, certs) }
        return reply.blacklistDigest()
    }

    /** Begin a direct exchange. Only the native nonce leaves clock custody. */
    fun beginTimeExchange(): KagemushaWalletTimeExchangeV1 {
        val reply = time(0)
        reply.requireShape(13, 32, token = true)
        val nonce = reply.bytes()
        if (nonce.all { it == 0.toByte() }) invalidSpartanOutput()
        return KagemushaWalletTimeExchangeV1(this, reply.sequenceLow, nonce)
    }

    /** Authenticate and durably retain the exact signed response and native observations. */
    fun finishTimeExchange(exchange: KagemushaWalletTimeExchangeV1, anchor: ByteArray, certificate: ByteArray) {
        require(anchor.size in 1..512 && certificate.size in 1..512)
        val token = exchange.tokenFor(this)
        owner.exchangeHandle()
        exchange.consume(this)
        time(1, token, anchor, certificate).requireShape(14, 0)
    }

    /** Retire the request without recording an observation. */
    fun cancelTimeExchange(exchange: KagemushaWalletTimeExchangeV1) {
        val token = exchange.tokenFor(this)
        owner.exchangeHandle()
        exchange.consume(this)
        time(2, token).requireShape(84, 0)
    }

    /** Freeze the Native signed-time interval for this time-controlled Send. */
    fun captureSendTime(operationId: ByteArray): KagemushaSpartanTimeIntervalV1 {
        require(operationId.size == 32)
        val reply = time(3, first = operationId)
        reply.requireShape(86, 16)
        return KagemushaSpartanTimeIntervalV1(reply.bytes())
    }

    /** Release an interval when preparation is abandoned before a Send review is available. */
    fun abandonSendCapture(operationId: ByteArray) {
        require(operationId.size == 32)
        time(4, first = operationId).requireShape(84, 0)
    }

    private fun time(selector: Int, token: Long = 0, first: ByteArray = byteArrayOf(), second: ByteArray = byteArrayOf()): KagemushaSpartanReplyV1 {
        val handle = owner.exchangeHandle()
        val a = first.copyOf()
        val b = second.copyOf()
        return call { NativeSpartanBridge.time(handle, slot.copyOf(), selector, token, a, b) }
    }

    /** Prepare the initial native wallet state from its account-authorized enrollment. */
    fun prepareBootstrap(): KagemushaSpartanReviewV1 = prepareOwned(KagemushaSpartanOperationV1.BOOTSTRAP)

    fun prepareLoad(receiptOriginal: ByteArray, finalityOriginal: ByteArray): KagemushaSpartanReviewV1 =
        prepareOwned(KagemushaSpartanOperationV1.LOAD, receiptOriginal, finalityOriginal)

    fun prepareSend(request: IrohaPeerWireMessageV1): KagemushaSpartanReviewV1 {
        require(request.canonicalPayload.profile == IrohaPeerPayloadProfile.KAGEMUSHA_SPARTAN_PAYMENT_V3 &&
            request.canonicalPayload.kind == IrohaPeerPayloadKind.REQUEST)
        return prepareOwned(KagemushaSpartanOperationV1.SEND, request.encode())
    }

    fun prepareReceive(requestId: ByteArray, payment: IrohaPeerWireMessageV1): KagemushaSpartanReviewV1 {
        require(requestId.size == 32 && requestId.any { it != 0.toByte() } &&
            payment.canonicalPayload.profile == IrohaPeerPayloadProfile.KAGEMUSHA_SPARTAN_PAYMENT_V3 &&
            payment.canonicalPayload.kind == IrohaPeerPayloadKind.PAYMENT)
        return prepareOwned(KagemushaSpartanOperationV1.RECEIVE, requestId, payment.encode())
    }

    /** Prepare one deferred ArchiveSent from the exact receipt retained for this Send. */
    fun prepareArchiveSent(sendOperationId: ByteArray, credited: IrohaPeerWireMessageV1): KagemushaSpartanReviewV1 {
        require(!retainedOutputsOnly)
        return prepareCommand(KagemushaSpartanOperationV1.ARCHIVE_SENT, archiveSentCommand(sendOperationId, credited))
    }

    private fun prepareOwned(operation: KagemushaSpartanOperationV1, vararg originals: ByteArray): KagemushaSpartanReviewV1 =
        prepareCommand(operation, ownedCommand(operation, *originals))

    private fun prepareCommand(operation: KagemushaSpartanOperationV1, command: ByteArray): KagemushaSpartanReviewV1 =
        prepare(command, byteArrayOf()) { token, bytes ->
            val projection = KagemushaSpartanReviewProjectionV1(bytes)
            if (projection.operation != operation) invalidSpartanOutput()
            KagemushaSpartanReviewV1(this, token, projection)
        }

    /** Native authenticates the complete witness and freezes a hardware-signable review. */
    fun prepare(witness: ByteArray, predecessorHead: ByteArray): KagemushaSpartanReviewV1 {
        return prepare(witness, predecessorHead) { token, bytes ->
            KagemushaSpartanReviewV1(this, token, KagemushaSpartanReviewProjectionV1(bytes))
        }
    }

    /** Prepare Native's signed anchor update for the wallet's ordinary durable Advance. */
    fun prepareRefreshTimeAnchor(witness: ByteArray, predecessorHead: ByteArray): KagemushaSpartanTimeAnchorReviewV2 {
        return prepare(witness, predecessorHead) { token, bytes ->
            KagemushaSpartanTimeAnchorReviewV2(this, token, KagemushaSpartanTimeAnchorProjectionV2(bytes))
        }
    }

    /**
     * Prepare Native's signed anchor update from the exact canonical anchor and TimeAnchor-role certificate
     * originals that [finishTimeExchange] accepts. Retry with the same originals. Only the signed floor can
     * advance: a time-controlled Send still requires Native's own direct exchange for the held anchor. The
     * private-witness [prepareRefreshTimeAnchor] has the same parameter types, hence the distinct name.
     */
    fun prepareRefreshTimeAnchorFromOriginals(anchor: ByteArray, certificate: ByteArray): KagemushaSpartanTimeAnchorReviewV2 =
        prepareRefreshTimeAnchor(refreshTimeAnchorCommand(anchor, certificate), byteArrayOf())

    /** Retain the exact reviewed Unload for its protected confirmation. */
    fun prepareUnload(witness: ByteArray, predecessorHead: ByteArray): KagemushaSpartanUnloadReviewV3 =
        prepare(witness, predecessorHead) { token, bytes ->
            KagemushaSpartanUnloadReviewV3(this, token, KagemushaSpartanUnloadProjectionV3(bytes))
        }

    /** Prepare an Unload to this account and preserve one request identity across restarts. */
    @JvmOverloads fun prepareUnload(requestId: ByteArray, amount: KagemushaWalletUInt128V1,
                                    quote: ByteArray = byteArrayOf(), certificates: ByteArray = byteArrayOf(),
                                    beneficiary: ByteArray = byteArrayOf()): KagemushaSpartanUnloadReviewV3 =
        prepareUnload(unloadCommand(requestId, amount, quote, certificates, beneficiary), byteArrayOf())

    /** Read the authenticated Unload terms and release the unconfirmed preparation. */
    fun reviewUnload(witness: ByteArray, predecessorHead: ByteArray): KagemushaSpartanUnloadProjectionV3 {
        return prepare(witness, predecessorHead) { token, bytes ->
            val projection = KagemushaSpartanUnloadProjectionV3(bytes)
            discard(token)
            projection
        }
    }

    private fun <T> prepare(witness: ByteArray, predecessorHead: ByteArray, decode: (Long, ByteArray) -> T): T {
        require(witness.isNotEmpty() && witness.size <= MAX_RECORD && predecessorHead.size <= 1_048_576)
        val identity = slot.copyOf()
        val privateInput = witness.copyOf()
        val head = predecessorHead.copyOf()
        try {
            val reply = call { NativeSpartanBridge.prepare(owner.exchangeHandle(), identity, privateInput, head) }
            reply.requireShape(82, null, token = true)
            return try {
                decode(reply.sequenceLow, reply.bytes())
            } catch (failure: Throwable) {
                try { discard(reply.sequenceLow) } catch (cleanup: Throwable) { failure.addSuppressed(cleanup) }
                throw failure
            }
        } finally { privateInput.fill(0) }
    }

    /** Reconcile durable selection and read the retained original. Retry only these exact Payment bytes. */
    fun output(operationId: ByteArray, part: KagemushaSpartanOutputV1): KagemushaSpartanRetainedOutputV1 {
        val operation = operationId.copyOf()
        require(operation.size == 32 && (part != KagemushaSpartanOutputV1.UNLOAD_CLAIM || operation.any { it != 0.toByte() }))
        val identity = slot.copyOf()
        val reply = call { NativeSpartanBridge.output(owner.exchangeHandle(), identity, operation, part.ordinal) }
        return KagemushaSpartanRetainedOutputV1.from(reply, part)
    }

    /** Read the wallet's selected offline native stage, and its spendable balance if Released. */
    fun snapshot(): KagemushaSpartanSnapshotV1 = KagemushaSpartanSnapshotV1(call {
        NativeSpartanBridge.output(owner.exchangeHandle(), slot.copyOf(), ByteArray(32), 3)
    })

    /** The current Released head's expected funding ordinal. Reading it does not reserve or fund a Load. */
    fun nextLoadOrdinal(): KagemushaWalletUInt128V1 = call {
        NativeSpartanBridge.output(owner.exchangeHandle(), slot.copyOf(), ByteArray(32), 4)
    }.nextLoadOrdinal()

    /** Issue or recover the same hardware-authorized registration of this wallet's Bootstrap. */
    fun bootstrapActivation(): KagemushaSpartanActivationV1 {
        require(!retainedOutputsOnly)
        return KagemushaSpartanActivationV1(call { NativeSpartanBridge.output(owner.exchangeHandle(), slot.copyOf(), ByteArray(32), 5) })
    }

    /** Reconstruct the receiver's authenticated delivery status from its protected Request and released Receive. */
    fun creditStatus(requestId: ByteArray, receiveOperationId: ByteArray): IrohaPeerWireMessageV1 {
        return receivedCreditStatus(requestId, receiveOperationId).message
    }

    /**
     * Read the actual receiver outcome together with the exact wire proving the protected opening. This
     * archive read names no Payment: it shows a selected Receive and is not the answer to a payer who just
     * delivered one.
     */
    fun receivedCreditStatus(requestId: ByteArray, receiveOperationId: ByteArray): KagemushaSpartanReceiverCreditStatusV1 {
        val request = requestId.copyOf()
        require(request.size == 32 && request.any { it != 0.toByte() })
        return KagemushaSpartanReceiverCreditStatusV1(creditExchange(1, receiveOperationId, request, byteArrayOf()))
    }

    /**
     * The receiver's status for exactly [payment], the answer to the payer who delivered it. Native compares
     * the complete IPM1 Payment, byte for byte, with the Payment retained in the released capsule of the
     * Receive [receiveOperationId] of the hosted Request [requestId], and answers that Receive's status only
     * for those bytes. Another Payment of the same Request, also one valid on its own, and an operation that
     * is not this wallet's released Receive of that Request are refused. The Receive is read from its own
     * durable capsule, so it answers also after later operations moved the wallet's head.
     */
    fun receivedCreditStatus(requestId: ByteArray, receiveOperationId: ByteArray,
                             payment: IrohaPeerWireMessageV1): KagemushaSpartanReceiverCreditStatusV1 {
        val request = requestId.copyOf()
        val operation = receiveOperationId.copyOf()
        val original = matchingPaymentOriginal(request, operation, payment)
        return KagemushaSpartanReceiverCreditStatusV1(creditExchange(3, operation, request, original))
    }

    /** Verify the receiver head against the exact Payment retained under the payer's released Send. */
    fun verifyCreditStatus(sendOperationId: ByteArray, status: IrohaPeerWireMessageV1): KagemushaSpartanCreditOutcomeV1 =
        verifyCreditStatus(sendOperationId, null, status)

    fun verifyCreditStatus(sendOperationId: ByteArray, request: IrohaPeerWireMessageV1?,
                           status: IrohaPeerWireMessageV1): KagemushaSpartanCreditOutcomeV1 {
        require(status.canonicalPayload.profile == IrohaPeerPayloadProfile.KAGEMUSHA_SPARTAN_PAYMENT_V3 &&
            status.canonicalPayload.kind == IrohaPeerPayloadKind.CREDITED)
        require(request == null || (request.canonicalPayload.profile == status.canonicalPayload.profile &&
            request.canonicalPayload.kind == IrohaPeerPayloadKind.REQUEST))
        return creditExchange(2, sendOperationId, request?.encode() ?: byteArrayOf(), status.encode()).verifiedCreditOutcome(status)
    }

    private fun creditExchange(selector: Int, operation: ByteArray, request: ByteArray, status: ByteArray): KagemushaSpartanReplyV1 {
        val op = operation.copyOf()
        require(op.size == 32 && op.any { it != 0.toByte() })
        return call { NativeSpartanBridge.creditStatus(owner.exchangeHandle(), slot.copyOf(), selector, op, request, status) }
    }

    /** Supply the exact durable Payment to a generic QR, NFC or Nearby carrier. */
    fun releasedPeerPayment(operationId: ByteArray): IrohaPeerWireMessageV1? {
        val released = output(operationId, KagemushaSpartanOutputV1.PAYMENT) as? KagemushaSpartanRetainedOutputV1.Released
            ?: return null
        val bytes = released.bytes()
        if (bytes.isEmpty()) return null
        return IrohaPeerWireMessageV1.decode(bytes, IrohaPeerPayloadProfile.KAGEMUSHA_SPARTAN_PAYMENT_V3,
            IrohaPeerPayloadKind.PAYMENT)
    }

    /** Verify the complete carrier output against this wallet's authenticated manifest. */
    fun verifyPayment(message: IrohaPeerWireMessageV1): KagemushaSpartanVerifiedPaymentV1 {
        require(message.canonicalPayload.profile == IrohaPeerPayloadProfile.KAGEMUSHA_SPARTAN_PAYMENT_V3 &&
            message.canonicalPayload.kind == IrohaPeerPayloadKind.PAYMENT)
        return verifyPayment(message.encode())
    }

    /** Verify a Payment and compare its completed Request to the receiver's exact hosted terms. */
    fun verifyPayment(message: IrohaPeerWireMessageV1, matching: IrohaPeerWireMessageV1): KagemushaSpartanVerifiedPaymentV1 {
        require(matching.canonicalPayload.profile == IrohaPeerPayloadProfile.KAGEMUSHA_SPARTAN_PAYMENT_V3 &&
            matching.canonicalPayload.kind == IrohaPeerPayloadKind.REQUEST)
        val hosted = IrohaPeerSpartanOpenRequestV3(matching.canonicalPayload.bytes)
        return verifyPayment(message).also { require(it.matches(hosted)) { "Payment differs from the hosted Request" } }
    }

    /** Full native proof verification; the returned Request is bound to these exact Payment bytes. */
    fun verifyPayment(payment: ByteArray): KagemushaSpartanVerifiedPaymentV1 {
        require(payment.isNotEmpty() && payment.size <= 65_536)
        val original = payment.copyOf()
        val reply = call { NativeSpartanBridge.verifyPayment(owner.exchangeHandle(), original) }
        reply.requireShape(83, 514)
        return KagemushaSpartanVerifiedPaymentV1(reply.bytes())
    }

    internal fun debitPrompt(token: Long, terms: (Int) -> String): KagemushaSpartanDebitPromptV1 {
        val platform = platformOwner as? KagemushaWalletAndroidPlatformV1 ?: invalidSpartanOutput()
        return createSpartanDebitPromptV1(platform, slot, terms) { message, der ->
            call { NativeSpartanBridge.confirm(owner.exchangeHandle(), token, message, der) }
        }
    }

    internal fun execute(token: Long): KagemushaSpartanExecutionV1 {
        val reply = call { NativeSpartanBridge.execute(owner.exchangeHandle(), token) }
        return KagemushaSpartanExecutionV1(reply)
    }

    internal fun discard(token: Long) {
        // Control cleanup must not queue behind the operation it cancels.
        checkedSpartanReply(NativeSpartanBridge.discard(owner.exchangeCleanupHandle(), token)).requireShape(84, 0)
    }

    /** Closing retains committed custody and exact outputs. */
    override fun close() = owner.close()

    companion object {
        internal fun ownedCommand(operation: KagemushaSpartanOperationV1, vararg originals: ByteArray): ByteArray {
            return ownedCommand(operation.ordinal + 1, *originals)
        }

        /**
         * The complete IPM1 original of the incoming [payment] that Native compares with a released Receive,
         * checked before Native is asked: both identities are 32 nonzero bytes, the message is an uncompressed
         * Spartan Payment, so it is its header and the exact canonical original, and it is within the carriers'
         * typed Payment bound ([IrohaPeerNfcV1.MAXIMUM_PAYMENT_BYTES]).
         */
        internal fun matchingPaymentOriginal(requestId: ByteArray, receiveOperationId: ByteArray,
                                             payment: IrohaPeerWireMessageV1): ByteArray {
            require(requestId.size == 32 && requestId.any { it != 0.toByte() })
            require(receiveOperationId.size == 32 && receiveOperationId.any { it != 0.toByte() })
            require(payment.canonicalPayload.profile == IrohaPeerPayloadProfile.KAGEMUSHA_SPARTAN_PAYMENT_V3 &&
                payment.canonicalPayload.kind == IrohaPeerPayloadKind.PAYMENT &&
                payment.encoding == IrohaPeerContentEncodingV1.NONE)
            val original = payment.encode()
            require(original.size == IrohaPeerWireMessageV1.HEADER_LENGTH + payment.canonicalPayload.byteCount &&
                original.size <= IrohaPeerNfcV1.MAXIMUM_PAYMENT_BYTES)
            return original
        }

        internal fun archiveSentCommand(sendOperationId: ByteArray, credited: IrohaPeerWireMessageV1): ByteArray {
            val operation = sendOperationId.copyOf()
            require(operation.size == 32 && operation.any { it != 0.toByte() })
            require(credited.canonicalPayload.profile == IrohaPeerPayloadProfile.KAGEMUSHA_SPARTAN_PAYMENT_V3 &&
                credited.canonicalPayload.kind == IrohaPeerPayloadKind.CREDITED && credited.encoding == IrohaPeerContentEncodingV1.NONE)
            require(credited.canonicalPayload.byteCount <= 10_084 - IrohaPeerWireMessageV1.HEADER_LENGTH)
            return ownedCommand(KagemushaSpartanOperationV1.ARCHIVE_SENT, operation, credited.encode())
        }

        internal fun unloadCommand(requestId: ByteArray, amount: KagemushaWalletUInt128V1,
                                   quote: ByteArray, certificates: ByteArray, beneficiary: ByteArray): ByteArray {
            require(requestId.size == 32 && requestId.any { it != 0.toByte() } && (amount.low != 0L || amount.high != 0L))
            val units = ByteBuffer.allocate(16).order(ByteOrder.LITTLE_ENDIAN).putLong(amount.low).putLong(amount.high).array()
            if (quote.isEmpty() && certificates.isEmpty() && beneficiary.isEmpty()) return ownedCommand(6, requestId, units)
            require(quote.size in 1..1024 && certificates.size in 1..10_034 && beneficiary.size in 1..4096)
            return ownedCommand(6, requestId, units, quote, certificates, beneficiary)
        }

        internal fun refreshTimeAnchorCommand(anchor: ByteArray, certificate: ByteArray): ByteArray {
            require(anchor.size in 1..512 && certificate.size in 1..512)
            return ownedCommand(7, anchor, certificate)
        }

        private fun ownedCommand(tag: Int, vararg originals: ByteArray): ByteArray {
            var size = 5
            for (original in originals) {
                require(original.isNotEmpty() && original.size <= MAX_RECORD - size - 4)
                size += 4 + original.size
            }
            val command = ByteBuffer.allocate(size).order(ByteOrder.LITTLE_ENDIAN)
                .put("KSO1".toByteArray(Charsets.US_ASCII)).put(tag.toByte())
            for (original in originals) command.putInt(original.size).put(original)
            return command.array()
        }
        internal const val MAX_RECORD = 4 * 1024 * 1024
        internal fun call(action: () -> KagemushaSpartanReplyV1?): KagemushaSpartanReplyV1 =
            checkedSpartanReply(KagemushaWalletNativeWorkerV1.shared.run { action() })
    }
}

/** Prepare one SDK-owned CryptoObject operation for an exact, live Native review. */
internal fun createSpartanDebitPromptV1(
    platform: KagemushaWalletAndroidPlatformV1,
    slot: ByteArray,
    terms: (Int) -> String,
    confirm: (ByteArray, ByteArray) -> KagemushaSpartanReplyV1,
): KagemushaSpartanDebitPromptV1 {
    platform.requireDebitAuthenticator()
    try { KagemushaSpartanAndroidConfirmationV1.provision(slot, platform.preferStrongBox) }
    catch (error: Exception) { throw KagemushaSpartanDebitUnavailableV1(
        KagemushaSpartanDebitUnavailableReasonV1.HARDWARE_CONFIRMATION_KEY_UNAVAILABLE, cause = error) }
    val empty = byteArrayOf()
    val asset = confirm(empty, byteArrayOf(2))
    asset.requireShape(94, 33)
    val scope = asset.bytes()
    val scale = scope[32].toInt() and 255
    if (scale > 28) invalidSpartanOutput()
    val display = "Asset ID ${spartanId(scope.copyOfRange(0, 32))}\n\n" + terms(scale)
    val begin = confirm(empty, empty)
    begin.requireShape(88, 32)
    val message = begin.bytes()
    val cancel = { original: ByteArray -> confirm(original, empty).requireShape(84, 0) }
    return try {
        val (operation, allowed) = KagemushaSpartanAndroidConfirmationV1.operation(slot)
        KagemushaSpartanDebitPromptV1(operation, message, display,
            { original, der -> platform.retainSpartanSignature(slot, original, der) { confirm(original, der).requireShape(89, 0) } },
            cancel, allowed)
    } catch (error: Exception) {
        try { cancel(message) } catch (cleanup: Exception) { error.addSuppressed(cleanup) }
        throw error
    }
}

/** Retained installed backend before the existing Ed25519 account signs Native's challenge. */
class KagemushaSpartanRuntimeV1 private constructor(
    private var owner: KagemushaWalletV1?,
    private val platformOwner: Any,
    parameterId: ByteArray,
) : Closeable {
    private val parameters = parameterId.copyOf()
    fun parameterId(): ByteArray = parameters.copyOf()
    /** Return the exact Native challenge. No payment key is created or replaced. */
    @Synchronized fun beginAccount(originals: KagemushaSpartanAccountOriginalsV1): ByteArray {
        val handle = requireNotNull(owner) { "Runtime was transferred or closed" }.exchangeHandle()
        val frames = originals.frames()
        val reply = KagemushaSpartanWalletV1.call { NativeSpartanBridge.beginAccount(handle, frames[0], frames[1], frames[2], frames[3]) }
        reply.requireShape(15, 32, token = true)
        val challenge = reply.bytes()
        if (reply.sequenceLow != handle || challenge.all { it == 0.toByte() }) invalidSpartanOutput()
        return challenge
    }
    /**
     * Return the exact Native challenge from one complete E6 enrollment result and the existing
     * account original. Native's signed selected owner supplies the asset scope and Enrollment
     * certificate itself and verifies the result against its retained E5, hardware key and
     * evidence before retaining anything. No payment key is created or replaced. An exact retry
     * returns the retained challenge. Continue with [authorizeAccount].
     */
    @Synchronized fun beginAccount(resultOriginal: ByteArray, accountOriginal: ByteArray): ByteArray {
        require(resultOriginal.isNotEmpty() && resultOriginal.size <= 262_144 && accountOriginal.isNotEmpty())
        val handle = requireNotNull(owner) { "Runtime was transferred or closed" }.exchangeHandle()
        // Same JNI entry and wire as the split form. The certificate and asset stay separate
        // inputs, and both empty select the complete-result form.
        val result = resultOriginal.copyOf()
        val account = accountOriginal.copyOf()
        val absent = ByteArray(0)
        val reply = KagemushaSpartanWalletV1.call { NativeSpartanBridge.beginAccount(handle, result, absent, account, absent) }
        reply.requireShape(15, 32, token = true)
        val challenge = reply.bytes()
        if (reply.sequenceLow != handle || challenge.all { it == 0.toByte() }) invalidSpartanOutput()
        return challenge
    }
    /** Finish actual Native account admission and transfer this exact custody owner once. */
    @Synchronized fun authorizeAccount(signature: ByteArray): KagemushaSpartanWalletV1 {
        require(signature.size == 64)
        val owner = requireNotNull(owner) { "Runtime was transferred or closed" }
        val handle = owner.exchangeHandle(); val original = signature.copyOf()
        val reply = KagemushaSpartanWalletV1.call { NativeSpartanBridge.authorizeAccount(handle, original) }
        val (slot, retainedOutputsOnly) = reply.authorizedSlot(handle, parameters)
        val wallet = KagemushaSpartanWalletV1(owner, parameters, slot, platformOwner, retainedOutputsOnly)
        this.owner = null
        return wallet
    }
    @Synchronized override fun close() { owner?.close(); owner = null }
    internal companion object {
        fun install(handle: Long, platformOwner: Any, programBundle: ByteArray,
            signedManifest: ByteArray, signature: ByteArray): KagemushaSpartanRuntimeV1 {
            require(programBundle.isNotEmpty() && programBundle.size <= 256 * 1024 * 1024)
            require(signedManifest.isNotEmpty() && signedManifest.size <= 16_384 && signature.size == 64)
            val programs = programBundle.copyOf(); val manifest = signedManifest.copyOf(); val signed = signature.copyOf()
            val reply = KagemushaSpartanWalletV1.call { NativeSpartanBridge.install(handle, programs, manifest, signed) }
            reply.requireShape(81, 32, token = true)
            if (reply.sequenceLow != handle) invalidSpartanOutput()
            // This private holder provides existing close/quarantine mechanics; it is never exposed as a legacy wallet.
            return KagemushaSpartanRuntimeV1(KagemushaWalletV1(handle), platformOwner, reply.bytes())
        }
        /** Open the programs Native already authenticated from this owner's installed signed carrier.
         * Three empty arrays are the one buffer-free form of the install entry: no program, manifest
         * or signature bytes are offered, and an owner without such programs refuses. */
        fun openInstalled(handle: Long, platformOwner: Any): KagemushaSpartanRuntimeV1 {
            val reply = KagemushaSpartanWalletV1.call {
                NativeSpartanBridge.install(handle, ByteArray(0), ByteArray(0), ByteArray(0))
            }
            val parameters = installedParameterId(reply, handle)
            // The same private holder as install: close/quarantine mechanics only.
            return KagemushaSpartanRuntimeV1(KagemushaWalletV1(handle), platformOwner, parameters)
        }
        /** The installed-owner reply is status 81 for this exact handle with a nonzero 32-byte identity. */
        fun installedParameterId(reply: KagemushaSpartanReplyV1, handle: Long): ByteArray {
            reply.requireShape(81, 32, token = true)
            val parameters = reply.bytes()
            if (reply.sequenceLow != handle || parameters.all { it == 0.toByte() }) invalidSpartanOutput()
            return parameters
        }
    }
}

/** Native signed-time bounds for the frozen operation, in Unix milliseconds. */
class KagemushaSpartanTimeIntervalV1 internal constructor(bytes: ByteArray) {
    val lowerMs: ULong
    val upperMs: ULong
    init {
        if (bytes.size != 16) invalidSpartanOutput()
        val fields = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN)
        lowerMs = fields.long.toULong()
        upperMs = fields.long.toULong()
        if (lowerMs > upperMs) invalidSpartanOutput()
    }
}

enum class KagemushaSpartanOutputV1 { HEAD, PAYMENT, COMPLETION, UNLOAD_CLAIM }
enum class KagemushaSpartanOperationV1 { BOOTSTRAP, LOAD, SEND, RECEIVE, ARCHIVE_SENT }

sealed class KagemushaSpartanRetainedOutputV1 {
    data object Unknown : KagemushaSpartanRetainedOutputV1()
    data object Pending : KagemushaSpartanRetainedOutputV1()
    data object Archived : KagemushaSpartanRetainedOutputV1()
    data object DeliveryLoss : KagemushaSpartanRetainedOutputV1()
    class Released internal constructor(bytes: ByteArray) : KagemushaSpartanRetainedOutputV1() {
        private val original = bytes.copyOf()
        fun bytes(): ByteArray = original.copyOf()
    }
    internal companion object {
        fun from(reply: KagemushaSpartanReplyV1, part: KagemushaSpartanOutputV1): KagemushaSpartanRetainedOutputV1 {
            reply.requireShape(reply.status, if (reply.status == 1) null else 0)
            return when (reply.status) {
                0 -> Unknown
                2 -> Pending
                4 -> Archived
                5 -> DeliveryLoss
                1 -> {
                    val bytes = reply.bytes()
                    // Completed Bootstrap, Load and Receive have no outgoing Payment.
                    if (bytes.isEmpty() && part != KagemushaSpartanOutputV1.PAYMENT) invalidSpartanOutput()
                    if (part == KagemushaSpartanOutputV1.UNLOAD_CLAIM && bytes.size > 16_384) invalidSpartanOutput()
                    Released(bytes)
                }
                else -> invalidSpartanOutput()
            }
        }
    }
}

/** Native's exact review, suitable for confirmation UI. No caller-supplied validity flag. */
class KagemushaSpartanReviewProjectionV1 internal constructor(bytes: ByteArray) {
    private val original = if (bytes.size == 291) bytes.copyOf() else invalidSpartanOutput()
    val operation: KagemushaSpartanOperationV1
    val amount: KagemushaWalletUInt128V1
    val fee: KagemushaWalletUInt128V1
    init {
        if (original[0] != 1.toByte() || original[1] != 0.toByte()) invalidSpartanOutput()
        operation = KagemushaSpartanOperationV1.entries.getOrNull((original[2].toInt() and 255) - 1) ?: invalidSpartanOutput()
        if (operation == KagemushaSpartanOperationV1.ARCHIVE_SENT && (131 until 227).any { original[it] != 0.toByte() }) {
            invalidSpartanOutput()
        }
        val view = ByteBuffer.wrap(original).order(ByteOrder.LITTLE_ENDIAN)
        amount = KagemushaWalletUInt128V1(view.getLong(131), view.getLong(139))
        fee = KagemushaWalletUInt128V1(view.getLong(147), view.getLong(155))
    }
    /** The complete Native review original for device confirmation. */
    fun bytes(): ByteArray = original.copyOf()
    fun schemeId(): ByteArray = original.copyOfRange(3, 35)
    fun walletId(): ByteArray = original.copyOfRange(35, 67)
    fun operationId(): ByteArray = original.copyOfRange(67, 99)
    fun capsuleDigest(): ByteArray = original.copyOfRange(99, 131)
    fun receiverWalletId(): ByteArray = original.copyOfRange(163, 195)
    fun receiverAccountId(): ByteArray = original.copyOfRange(195, 227)
    fun beforeHead(): ByteArray = original.copyOfRange(227, 259)
    fun afterHead(): ByteArray = original.copyOfRange(259, 291)
    override fun toString(): String = "KagemushaSpartanReviewProjectionV1([REDACTED])"
}

/** Authenticated Native fields for RefreshTimeAnchor under review wire version 2. */
class KagemushaSpartanTimeAnchorProjectionV2 internal constructor(bytes: ByteArray) {
    private val original = bytes.copyOf()
    val issuerTime: ULong
    val successorFloor: ULong
    init {
        if (original.size != 243 || original[0] != 2.toByte() || original[1] != 0.toByte() ||
            original[2] != 7.toByte()) invalidSpartanOutput()
        val view = ByteBuffer.wrap(original).order(ByteOrder.LITTLE_ENDIAN)
        issuerTime = view.getLong(163).toULong()
        successorFloor = view.getLong(171).toULong()
        if (successorFloor < issuerTime) invalidSpartanOutput()
    }
    fun schemeId(): ByteArray = original.copyOfRange(3, 35)
    fun walletId(): ByteArray = original.copyOfRange(35, 67)
    fun operationId(): ByteArray = original.copyOfRange(67, 99)
    fun capsuleDigest(): ByteArray = original.copyOfRange(99, 131)
    fun anchorDigest(): ByteArray = original.copyOfRange(131, 163)
    fun beforeHead(): ByteArray = original.copyOfRange(179, 211)
    fun afterHead(): ByteArray = original.copyOfRange(211, 243)
    override fun toString(): String = "KagemushaSpartanTimeAnchorProjectionV2([REDACTED])"
}

/** Native authenticates these destination bytes against the credential and the signed charge quote. */
class KagemushaSpartanUnloadProjectionV3 internal constructor(bytes: ByteArray) {
    private val original = if (bytes.size in 423..16_384) bytes.copyOf() else invalidSpartanOutput()
    val ordinal: KagemushaWalletUInt128V1
    val faceDebit: KagemushaWalletUInt128V1
    val onlineCharge: KagemushaWalletUInt128V1
    val netToAccount: KagemushaWalletUInt128V1
    private val account: ByteArray
    private val beneficiary: ByteArray?
    init {
        if (original[0] != 3.toByte() || original[1] != 0.toByte() || original[2] != 6.toByte()) invalidSpartanOutput()
        val view = ByteBuffer.wrap(original).order(ByteOrder.LITTLE_ENDIAN)
        fun scalar(i: Int) = KagemushaWalletUInt128V1(view.getLong(i), view.getLong(i + 8))
        ordinal = scalar(195)
        faceDebit = scalar(211)
        onlineCharge = scalar(227)
        netToAccount = scalar(243)
        view.position(419)
        fun address(): ByteArray {
            if (view.remaining() < 2) invalidSpartanOutput()
            val size = view.short.toInt() and 0xffff
            if (size > view.remaining()) invalidSpartanOutput()
            return ByteArray(size).also { view.get(it) }
        }
        account = address()
        val quoted = address()
        val zero = KagemushaWalletUInt128V1(0, 0)
        val charged = onlineCharge != zero
        if (view.hasRemaining() || account.isEmpty() || nullifier().all { it == 0.toByte() } ||
            faceDebit == zero || onlineCharge > faceDebit || charged != quoted.isNotEmpty() ||
            charged != quoteDigest().any { it != 0.toByte() } ||
            charged != beneficiaryAccountDigest().any { it != 0.toByte() }) invalidSpartanOutput()
        val borrow = if (faceDebit.low.toULong() < onlineCharge.low.toULong()) 1L else 0L
        if (netToAccount != KagemushaWalletUInt128V1(faceDebit.low - onlineCharge.low,
                faceDebit.high - onlineCharge.high - borrow)) invalidSpartanOutput()
        beneficiary = if (charged) quoted else null
    }
    /** The complete Native review original, including both destination address frames. */
    fun bytes(): ByteArray = original.copyOf()
    fun schemeId(): ByteArray = original.copyOfRange(3, 35)
    fun walletId(): ByteArray = original.copyOfRange(35, 67)
    fun operationId(): ByteArray = original.copyOfRange(67, 99)
    fun capsuleDigest(): ByteArray = original.copyOfRange(99, 131)
    fun assetDigest(): ByteArray = original.copyOfRange(131, 163)
    fun nullifier(): ByteArray = original.copyOfRange(163, 195)
    fun quoteDigest(): ByteArray = original.copyOfRange(259, 291)
    fun credentialAccountDigest(): ByteArray = original.copyOfRange(291, 323)
    fun beneficiaryAccountDigest(): ByteArray = original.copyOfRange(323, 355)
    fun beforeHead(): ByteArray = original.copyOfRange(355, 387)
    fun afterHead(): ByteArray = original.copyOfRange(387, 419)
    fun credentialAccountBytes(): ByteArray = account.copyOf()
    fun beneficiaryAccountBytes(): ByteArray? = beneficiary?.copyOf()
    override fun toString(): String = "KagemushaSpartanUnloadProjectionV3([REDACTED])"
}

class KagemushaSpartanReviewV1 internal constructor(
    owner: KagemushaSpartanWalletV1, token: Long, val projection: KagemushaSpartanReviewProjectionV1,
) : KagemushaSpartanPreparedReviewV1(owner, token) {
    fun beginDebitConfirmation(): KagemushaSpartanDebitPromptV1 {
        return debitPrompt(projection.sendDebitTerms())
    }
}

internal fun KagemushaSpartanReviewProjectionV1.sendDebitTerms(): (Int) -> String {
    require(operation == KagemushaSpartanOperationV1.SEND)
    val atoms = spartanAtoms(amount)
    val feeAtoms = spartanAtoms(fee)
    val total = atoms + feeAtoms
    require(total.bitLength() <= 128)
    return { scale ->
        "Send ${spartanScaledAmount(atoms, scale)} asset units\nFee  ${spartanScaledAmount(feeAtoms, scale)}" +
        "\nTotal debit  ${spartanScaledAmount(total, scale)}" +
        "\n\nReceiving wallet ID\n${spartanId(receiverWalletId())}" +
        "\n\nReceiving account ID\n${spartanId(receiverAccountId())}"
    }
}

class KagemushaSpartanUnloadReviewV3 internal constructor(
    owner: KagemushaSpartanWalletV1, token: Long, val projection: KagemushaSpartanUnloadProjectionV3,
) : KagemushaSpartanPreparedReviewV1(owner, token) {
    fun beginDebitConfirmation(): KagemushaSpartanDebitPromptV1 {
        return debitPrompt { scale ->
            fun value(units: KagemushaWalletUInt128V1) = spartanScaledAmount(spartanAtoms(units), scale)
            val beneficiary = projection.beneficiaryAccountBytes()?.let { "\n\nCharge beneficiary account\n${spartanId(it)}" }.orEmpty()
            "Move ${value(projection.faceDebit)} asset units online\nAccount receives  ${value(projection.netToAccount)}" +
                "\nCharge  ${value(projection.onlineCharge)}\n\nReceiving account\n${spartanId(projection.credentialAccountBytes())}" + beneficiary
        }
    }
}

internal fun spartanScaledAmount(atoms: java.math.BigInteger, scale: Int): String =
    java.math.BigDecimal(atoms, scale).toPlainString()

private fun spartanAtoms(value: KagemushaWalletUInt128V1): java.math.BigInteger {
    val bytes = java.nio.ByteBuffer.allocate(16).putLong(value.high).putLong(value.low).array()
    return java.math.BigInteger(1, bytes)
}
private fun spartanId(value: ByteArray): String = value.joinToString("") { "%02x".format(it) }

class KagemushaSpartanTimeAnchorReviewV2 internal constructor(
    owner: KagemushaSpartanWalletV1, token: Long, val projection: KagemushaSpartanTimeAnchorProjectionV2,
) : KagemushaSpartanPreparedReviewV1(owner, token)

/** Review permanently bound to one Native owner and capsule; pending execution is retryable. */
sealed class KagemushaSpartanPreparedReviewV1 protected constructor(
    private val owner: KagemushaSpartanWalletV1, token: Long,
) : Closeable {
    private val retained = AtomicLong(token)
    protected fun debitPrompt(terms: (Int) -> String): KagemushaSpartanDebitPromptV1 {
        val token = retained.get()
        check(token > 0) { "Review is consumed or executing" }
        return owner.debitPrompt(token, terms)
    }
    /** Advance the frozen operation. Installed custody refuses Send until it can verify protected confirmation. */
    fun execute(): KagemushaSpartanExecutionV1 {
        val token = retained.get()
        check(token > 0 && retained.compareAndSet(token, -token)) { "Review is consumed or executing" }
        try {
            val result = owner.execute(token)
            retained.set(if (result.status == KagemushaSpartanExecutionV1.Status.PENDING) token else 0)
            return result
        } catch (failure: Throwable) {
            // A failed transport/allocation may hide Selected. Only retry the same frozen token.
            retained.set(token)
            throw failure
        }
    }
    override fun close() {
        val token = retained.get()
        if (token == 0L) return
        check(token > 0 && retained.compareAndSet(token, -token)) { "Review is executing" }
        try { owner.discard(token); retained.set(0) }
        catch (failure: Throwable) { retained.set(token); throw failure }
    }
}

/** Uncertain statuses require recovery of the same operation, never a replacement debit. */
class KagemushaSpartanExecutionV1 internal constructor(reply: KagemushaSpartanReplyV1) {
    enum class Status { RELEASED, PENDING, NOT_PERFORMED, ARCHIVED, DELIVERY_LOSS }
    val status = Status.entries.getOrNull(reply.status - 1) ?: invalidSpartanOutput()
    val sequence = KagemushaWalletUInt128V1(reply.sequenceLow, reply.sequenceHigh)
    val detail = reply.detail
    private val retained = reply.bytes()
    init {
        // Native Execute reserves both sequence words. Only NOT_PERFORMED carries a reason.
        val validDetail = if (status == Status.NOT_PERFORMED) detail in 0..2 else detail == 0
        if (reply.sequenceLow != 0L || reply.sequenceHigh != 0L || !validDetail ||
            (status == Status.RELEASED) != retained.isNotEmpty()) invalidSpartanOutput()
    }
    fun completion(): ByteArray? = retained.takeIf { status == Status.RELEASED }?.copyOf()
}

/** No amount is available on a pending or newly enrolled wallet. */
class KagemushaSpartanSnapshotV1 internal constructor(reply: KagemushaSpartanReplyV1) {
    sealed interface Stage {
        object Enrolled : Stage
        class Pending internal constructor(private val operation: ByteArray) : Stage {
            fun operationId() = operation.copyOf()
        }
        class Released internal constructor(private val operation: ByteArray, private val selectedHead: ByteArray,
            val sequence: KagemushaWalletUInt128V1, val balance: KagemushaWalletUInt128V1,
            val burned: KagemushaWalletUInt128V1, val spendable: KagemushaWalletUInt128V1) : Stage {
            fun operationId() = operation.copyOf()
            fun head() = selectedHead.copyOf()
        }
    }
    val scale: Int
    val stage: Stage
    private val wallet: ByteArray
    private val asset: ByteArray
    fun walletId() = wallet.copyOf()
    fun assetDigest() = asset.copyOf()
    init {
        reply.requireShape(101, null)
        val b = reply.bytes()
        if (b.size < 68 || b[0] != 1.toByte() || b[1] != 0.toByte() || b[3].toInt() !in 0..28) invalidSpartanOutput()
        val words = ByteBuffer.wrap(b).order(ByteOrder.LITTLE_ENDIAN)
        fun amount(at: Int) = KagemushaWalletUInt128V1(words.getLong(at),words.getLong(at+8))
        fun id(at: Int) = b.copyOfRange(at,at+32).also { if (it.all { v -> v == 0.toByte() }) invalidSpartanOutput() }
        wallet = id(4); asset = id(36); scale = b[3].toInt()
        stage = when {
            b[2] == 1.toByte() && b.size == 68 -> Stage.Enrolled
            b[2] == 2.toByte() && b.size == 100 -> Stage.Pending(id(68))
            b[2] == 3.toByte() && b.size == 196 -> {
                val balance = amount(148); val burned = amount(164); val spendable = amount(180)
                val borrow = if (java.lang.Long.compareUnsigned(balance.low,burned.low) < 0) 1L else 0L
                if (balance < burned || spendable.low != balance.low - burned.low ||
                    spendable.high != balance.high - burned.high - borrow) invalidSpartanOutput()
                Stage.Released(id(68),id(100),amount(132),balance,burned,spendable)
            }
            else -> invalidSpartanOutput()
        }
    }
}

class KagemushaSpartanVerifiedPaymentV1 internal constructor(bytes: ByteArray) {
    private val original = bytes.copyOf()
    init { if (original.size != 514) invalidSpartanOutput() }
    fun digest(): ByteArray = original.copyOfRange(0, 32)
    fun request(): ByteArray = original.copyOfRange(32, 514)
    internal fun matches(hosted: IrohaPeerSpartanOpenRequestV3): Boolean {
        val open = hosted.request()
        return (0 until 66).all { original[32+it] == open[it] } &&
            (130 until 194).all { original[32+it] == open[it] } && (210 until 482).all { original[32+it] == open[it] }
    }
}

// Distinct JNI domain: the production wallet status contract is unchanged.
/** An outcome of Native's complete receiver proof verification against the retained Send. */
enum class KagemushaSpartanCreditOutcomeV1 { CREDITED, BURNED }

/** An immutable receiver decision derived from the native wallet's released Receive and protected opening. */
class KagemushaSpartanReceiverCreditStatusV1 internal constructor(reply: KagemushaSpartanReplyV1) {
    val message: IrohaPeerWireMessageV1 = reply.issuedCreditStatus()
    val outcome: KagemushaSpartanCreditOutcomeV1 = if (!message.canonicalPayload.spartanCreditStatusBurned())
        KagemushaSpartanCreditOutcomeV1.CREDITED else KagemushaSpartanCreditOutcomeV1.BURNED
}

internal class KagemushaSpartanReplyV1(
    @JvmField val status: Int, @JvmField val reason: Int, @JvmField val platformCode: Int,
    @JvmField val sequenceLow: Long, @JvmField val sequenceHigh: Long, @JvmField val detail: Int,
    bytes: ByteArray,
) {
    private val retained = bytes.copyOf()
    fun bytes(): ByteArray = retained.copyOf()
    fun issuedOpenRequest(amount: ByteArray, feeSchedule: ByteArray): IrohaPeerWireMessageV1 {
        requireShape(96, null)
        val message = try {
            IrohaPeerWireMessageV1.decode(bytes(), IrohaPeerPayloadProfile.KAGEMUSHA_SPARTAN_PAYMENT_V3, IrohaPeerPayloadKind.REQUEST)
        } catch (_: IllegalArgumentException) { invalidSpartanOutput() }
        val originals = IrohaPeerSpartanOpenRequestV3(message.canonicalPayload.bytes)
        if (!originals.request().copyOfRange(242, 258).contentEquals(amount) ||
            !originals.feeSchedule().contentEquals(feeSchedule)) invalidSpartanOutput()
        return message
    }
    fun issuedCreditStatus(): IrohaPeerWireMessageV1 {
        requireShape(104, null)
        return try {
            IrohaPeerWireMessageV1.decode(retained, IrohaPeerPayloadProfile.KAGEMUSHA_SPARTAN_PAYMENT_V3,
                IrohaPeerPayloadKind.CREDITED)
        } catch (_: IllegalArgumentException) { invalidSpartanOutput() }
    }

    fun verifiedCreditOutcome(message: IrohaPeerWireMessageV1): KagemushaSpartanCreditOutcomeV1 {
        requireShape(105, 1)
        require(message.canonicalPayload.profile == IrohaPeerPayloadProfile.KAGEMUSHA_SPARTAN_PAYMENT_V3 &&
            message.canonicalPayload.kind == IrohaPeerPayloadKind.CREDITED)
        val burned = message.canonicalPayload.spartanCreditStatusBurned()
        return when {
            retained[0] == 1.toByte() && !burned -> KagemushaSpartanCreditOutcomeV1.CREDITED
            retained[0] == 0.toByte() && burned -> KagemushaSpartanCreditOutcomeV1.BURNED
            else -> invalidSpartanOutput()
        }
    }

    fun blacklistDigest(): ByteArray {
        requireShape(95,32)
        if (retained.all { it == 0.toByte() }) invalidSpartanOutput()
        return retained.copyOf()
    }
    fun authorizedSlot(handle: Long, parameters: ByteArray): Pair<ByteArray, Boolean> {
        if (status != 85 && status != 87) invalidSpartanOutput()
        requireShape(status, 64, token = true)
        val slot = retained.copyOfRange(32, 64)
        if (sequenceLow != handle || !retained.copyOfRange(0, 32).contentEquals(parameters) ||
            slot.all { it == 0.toByte() }) invalidSpartanOutput()
        return slot to (status == 87)
    }
    fun nextLoadOrdinal(): KagemushaWalletUInt128V1 {
        requireShape(106, 16)
        val words = ByteBuffer.wrap(retained).order(ByteOrder.LITTLE_ENDIAN)
        return KagemushaWalletUInt128V1(words.long, words.long)
    }

    fun requireShape(expectedStatus: Int, count: Int?, token: Boolean = false) {
        if (status != expectedStatus || (count != null && retained.size != count) || sequenceHigh != 0L || detail != 0 ||
            (if (token) sequenceLow <= 0 else sequenceLow != 0L)) invalidSpartanOutput()
    }
    fun checked(): KagemushaSpartanReplyV1 {
        if (status < 0) throw KagemushaWalletExceptionV1(status, reason, platformCode)
        if (reason != -1 || platformCode != 0 || retained.size > KagemushaSpartanWalletV1.MAX_RECORD) invalidSpartanOutput()
        return this
    }
}
private fun checkedSpartanReply(reply: KagemushaSpartanReplyV1?): KagemushaSpartanReplyV1 = reply?.checked() ?: invalidSpartanOutput()
private fun invalidSpartanOutput(): Nothing = throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)

internal object NativeSpartanBridge {
    @JvmStatic external fun activationSettlement(handle: Long, slot: ByteArray, signedTransaction: ByteArray, proof: ByteArray): KagemushaSpartanReplyV1?
    @JvmStatic external fun unloadSettlement(handle: Long, slot: ByteArray, operationId: ByteArray, signedTransaction: ByteArray, proof: ByteArray): KagemushaSpartanReplyV1?
    @JvmStatic external fun creditStatus(handle: Long, slot: ByteArray, selector: Int, operation: ByteArray,
                                         request: ByteArray, status: ByteArray): KagemushaSpartanReplyV1?
    @JvmStatic external fun epoch(handle: Long, slot: ByteArray, expectedEpoch: Long, boundary: ByteArray): KagemushaSpartanReplyV1?
    @JvmStatic external fun install(handle: Long, bundle: ByteArray, manifest: ByteArray, signature: ByteArray): KagemushaSpartanReplyV1?
    @JvmStatic external fun issueOpenRequest(handle: Long, slot: ByteArray, requestId: ByteArray, amountLe: ByteArray,
        feeSchedule: ByteArray, feeCertificate: ByteArray): KagemushaSpartanReplyV1?
    @JvmStatic external fun installBlacklist(handle: Long, slot: ByteArray, original: ByteArray, certificates: ByteArray): KagemushaSpartanReplyV1?
    @JvmStatic external fun confirm(handle: Long, token: Long, message: ByteArray, der: ByteArray): KagemushaSpartanReplyV1?
    @JvmStatic external fun time(handle: Long, slot: ByteArray, selector: Int, token: Long, first: ByteArray, second: ByteArray): KagemushaSpartanReplyV1?
    @JvmStatic external fun prepare(handle: Long, slot: ByteArray, witness: ByteArray, head: ByteArray): KagemushaSpartanReplyV1?
    @JvmStatic external fun execute(handle: Long, token: Long): KagemushaSpartanReplyV1?
    @JvmStatic external fun output(handle: Long, slot: ByteArray, operation: ByteArray, part: Int): KagemushaSpartanReplyV1?
    @JvmStatic external fun verifyPayment(handle: Long, payment: ByteArray): KagemushaSpartanReplyV1?
    @JvmStatic external fun discard(handle: Long, token: Long): KagemushaSpartanReplyV1?
    @JvmStatic external fun beginAccount(handle: Long, credential: ByteArray, certificate: ByteArray, account: ByteArray, asset: ByteArray): KagemushaSpartanReplyV1?
    @JvmStatic external fun authorizeAccount(handle: Long, signature: ByteArray): KagemushaSpartanReplyV1?
}

/** Certified transaction identity and any pending validator-epoch synchronization target. */
sealed interface KagemushaSpartanCertifiedAttemptV1 {
    val height: java.math.BigInteger
    val accepted: Boolean
    val requiredHeight: java.math.BigInteger?
    fun entrypoint(): ByteArray
    fun block(): ByteArray
}

/** Native's certified outcome for one exact envelope and the durable payout of its released Unload. */
class KagemushaSpartanUnloadSettlementV1 internal constructor(reply: KagemushaSpartanReplyV1, expectedStatus: Int = 102) : KagemushaSpartanCertifiedAttemptV1 {
    private val data: ByteArray
    override val height: java.math.BigInteger
    override val accepted: Boolean
    val paid: Boolean
    val requiredEpoch: java.math.BigInteger?
    /** Unverified synchronization target until Native verifies the offered certificate. */
    override val requiredHeight: java.math.BigInteger?
    override fun entrypoint(): ByteArray = data.copyOfRange(0, 32)
    override fun block(): ByteArray = data.copyOfRange(40, 72)

    init {
        reply.requireShape(expectedStatus, 83)
        data = reply.bytes()
        val wireHeight = settlementWord(data, 32)
        height = if (data[74].toInt() == 1) java.math.BigInteger.ZERO else wireHeight
        requiredHeight = if (data[74].toInt() == 1) wireHeight else null
        accepted = data[72].toInt() == 1
        paid = data[73].toInt() == 1
        val epoch = settlementWord(data, 75)
        requiredEpoch = if (data[74].toInt() == 1) epoch else null
        if ((72..74).any { data[it].toInt() !in 0..1 } || entrypoint().all { it.toInt() == 0 } ||
            (height.signum() == 0) != block().all { it.toInt() == 0 } || (accepted && (!paid || height.signum() == 0)) ||
            (requiredEpoch == null && epoch.signum() != 0) || (requiredHeight != null && wireHeight.signum() == 0)) invalidSpartanOutput()
    }
}

/** Scope and exact signed claim issued by Native for the authorized immutable Bootstrap. */
class KagemushaSpartanActivationV1 internal constructor(reply: KagemushaSpartanReplyV1) {
    private val bytes = reply.bytes()
    init {
        reply.requireShape(107, null)
        if (bytes.size !in 129..(128 + 32_768) ||
            (0..3).any { index -> (index * 32 until (index + 1) * 32).all { bytes[it] == 0.toByte() } }) invalidSpartanOutput()
    }
    fun schemeId() = bytes.copyOfRange(0, 32)
    fun assetDigest() = bytes.copyOfRange(32, 64)
    fun manifestDigest() = bytes.copyOfRange(64, 96)
    fun walletId() = bytes.copyOfRange(96, 128)
    fun original() = bytes.copyOfRange(128, bytes.size)
    fun instruction() = org.hyperledger.iroha.sdk.core.model.instructions.KagemushaSpartanActivateInstructionV1(
        schemeId(), assetDigest(), manifestDigest(), walletId(), original())
}

/** Only a verified certified Activate can set the wallet's durable activated bit. */
class KagemushaSpartanActivationSettlementV1 internal constructor(reply: KagemushaSpartanReplyV1) : KagemushaSpartanCertifiedAttemptV1 {
    private val value = KagemushaSpartanUnloadSettlementV1(reply, 108)
    override val height get() = value.height
    override val accepted get() = value.accepted
    val activated get() = value.paid
    val requiredEpoch get() = value.requiredEpoch
    override val requiredHeight get() = value.requiredHeight
    override fun entrypoint() = value.entrypoint()
    override fun block() = value.block()
}

private fun settlementWord(bytes: ByteArray, offset: Int) =
    java.math.BigInteger(1, bytes.copyOfRange(offset, offset + 8).reversedArray())
