// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import kotlin.test.fail

/**
 * Engineering fixtures of the Spartan profile for the carrier: a structurally valid open Request naming a
 * scheme and an asset, a Payment carrier and a CreditStatus of the sizes real ones have. They are shapes
 * only. No signature or proof in them is valid, which is the point: under this profile the carrier binds
 * nothing by shape, and every test below passes or fails by what the mock wallet decides.
 */
internal object IrohaPeerNfcSpartanFixturesV1 {
    val PROFILE = IrohaPeerPayloadProfile.KAGEMUSHA_SPARTAN_PAYMENT_V3
    val SCHEME = ByteArray(32) { 0x51 }
    val ASSET = ByteArray(32) { 0x52 }

    /** The IPM1 size of a real Payment and of a real CreditStatus measured on the current scheme. */
    const val REAL_PAYMENT_BYTES = 10_711
    const val REAL_STATUS_BYTES = 9_400

    private fun be32(frame: ByteArray, offset: Int, value: Int) {
        for (index in 0..3) frame[offset + index] = (value ushr (24 - 8 * index)).toByte()
    }

    private fun wrap(kind: IrohaPeerPayloadKind, payload: ByteArray) =
        IrohaPeerWireMessageV1(IrohaPeerCanonicalPayload(PROFILE, kind, PROFILE.requiredSchemaVersion, payload))

    /** An open Request without a fee for [scheme] and [asset]; [nonce] makes it another Request. */
    fun request(scheme: ByteArray = SCHEME, asset: ByteArray = ASSET, nonce: Int = 4): IrohaPeerWireMessageV1 {
        val frame = ByteArray(16 + 1_250 + 1)
        "KWRQAC1\u0000".toByteArray(Charsets.US_ASCII).copyInto(frame)
        be32(frame, 8, 1_250)
        be32(frame, 12, 1)
        frame[16] = 1
        scheme.copyInto(frame, 16 + 2)
        asset.copyInto(frame, 16 + 34)
        frame[16 + 242] = 7
        frame[16 + 386] = nonce.toByte()
        frame[16 + 418] = 9
        // The receiver credential and the certificate repeat the Request's root selection.
        frame.copyInto(frame, 498, 16, 16 + 66)
        frame[498 + 130] = 4
        frame.copyInto(frame, 1_094, 16, 16 + 34)
        frame[1_094 + 34] = 1
        frame[1_094 + 35] = 4
        frame[frame.lastIndex] = 97
        return wrap(IrohaPeerPayloadKind.REQUEST, frame)
    }

    /** A Payment carrier whose complete IPM1 message is [messageBytes] long. */
    fun payment(messageBytes: Int = REAL_PAYMENT_BYTES, seed: Int = 3): IrohaPeerWireMessageV1 {
        val canonical = messageBytes - IrohaPeerWireMessageV1.HEADER_LENGTH
        val companion = minOf(2_048, canonical - 16 - 1)
        val envelope = canonical - 16 - companion
        val frame = ByteArray(canonical) { (it * 31 + seed).toByte() }
        "KWPYPC1\u0000".toByteArray(Charsets.US_ASCII).copyInto(frame)
        be32(frame, 8, envelope)
        be32(frame, 12, companion)
        return wrap(IrohaPeerPayloadKind.PAYMENT, frame)
    }

    /** A CreditStatus whose complete IPM1 message is [messageBytes] long: empty fields, then its folded pair. */
    fun status(messageBytes: Int = REAL_STATUS_BYTES, burned: Boolean = false, seed: Int = 7): IrohaPeerWireMessageV1 {
        val canonical = messageBytes - IrohaPeerWireMessageV1.HEADER_LENGTH
        val fixed = 8 + 1 + 9 + 160 + 6 + 8
        val frame = ByteArray(canonical)
        "KGSCST3\u0000".toByteArray(Charsets.US_ASCII).copyInto(frame)
        frame[8] = if (burned) 1 else 0
        for (index in fixed until canonical) frame[index] = (index * 17 + seed).toByte()
        return wrap(IrohaPeerPayloadKind.CREDITED, frame)
    }

    fun card(
        wallet: IrohaPeerNfcSpartanMockWalletV1,
        request: IrohaPeerWireMessageV1,
        session: ByteArray = ByteArray(16) { 0x6b },
    ): IrohaPeerNfcCardEndpointV1 = IrohaPeerNfcCardEndpointV1.direct(
        IrohaPeerNfcTestFixturesV1.BPNG_AID,
        IrohaPeerNfcReceiverSessionV1(session, request.encode(), profilePolicy = wallet.policy),
    )
}

/**
 * A wallet as the carrier sees one: only what it decides. It holds a signed selection, the Payment it
 * retains for each Request it paid, the one status its verification accepts for each of those Payments, and
 * the status it issued for each Request it was paid for. Nothing is accepted by reading a message.
 */
internal class IrohaPeerNfcSpartanMockWalletV1(
    private val scheme: ByteArray = IrohaPeerNfcSpartanFixturesV1.SCHEME,
    private val asset: ByteArray = IrohaPeerNfcSpartanFixturesV1.ASSET,
) : IrohaPeerNfcExchangeVerifierV1 {
    val policy = IrohaPeerNfcProfilePolicyV1(IrohaPeerNfcSpartanFixturesV1.PROFILE, this)
    val calls = mutableListOf<String>()

    /** Where this wallet's status verifications are also written, to order them against the carrier's commands. */
    var journal: MutableList<String>? = null

    private val sent = HashMap<String, ByteArray>()
    private val verified = HashMap<String, ByteArray>()
    private val issued = HashMap<String, ByteArray>()

    private fun key(hash: ByteArray) = hash.joinToString("") { "%02x".format(it.toInt() and 0xff) }

    /** The protected Send of [request] released exactly [payment]. */
    fun send(request: IrohaPeerWireMessageV1, payment: IrohaPeerWireMessageV1) { sent[key(request.wireHash)] = payment.encode() }

    /** This wallet's verification of [status] against its retained [payment] succeeds, and of no other status. */
    fun accept(payment: IrohaPeerWireMessageV1, status: IrohaPeerWireMessageV1) { verified[key(payment.wireHash)] = status.encode() }

    /** This wallet received the Payment of [request] and retains [status] as its decision. */
    fun issue(request: IrohaPeerWireMessageV1, status: IrohaPeerWireMessageV1) { issued[key(request.wireHash)] = status.encode() }

    fun count(call: String): Int = calls.count { it == call }

    override fun requireRequest(request: IrohaPeerWireMessageV1) {
        calls += "request"
        val body = IrohaPeerSpartanOpenRequestV3(request.canonicalPayload.bytes).request()
        require(body.copyOfRange(2, 34).contentEquals(scheme) && body.copyOfRange(34, 66).contentEquals(asset)) {
            "Request outside the signed selection"
        }
    }

    override fun requireOutgoingPayment(request: IrohaPeerWireMessageV1, payment: IrohaPeerWireMessageV1) {
        calls += "payment"
        require(sent[key(request.wireHash)]?.contentEquals(payment.encode()) == true) { "not the retained Payment of this Request" }
    }

    override fun requireReceivedStatus(
        request: IrohaPeerWireMessageV1,
        payment: IrohaPeerWireMessageV1,
        status: IrohaPeerWireMessageV1,
    ) {
        calls += "status"
        journal?.add("verify:status")
        require(sent[key(request.wireHash)]?.contentEquals(payment.encode()) == true &&
            verified[key(payment.wireHash)]?.contentEquals(status.encode()) == true) { "status verification failed" }
    }

    override fun requireIssuedStatus(
        request: IrohaPeerWireMessageV1,
        payment: IrohaPeerWireMessageV1?,
        status: IrohaPeerWireMessageV1,
    ) {
        calls += "issued"
        require(issued[key(request.wireHash)]?.contentEquals(status.encode()) == true) { "not this wallet's status" }
    }
}

/**
 * The Spartan profile over the same in-memory card and APDU codec as the wallet V1 exchange: the carrier
 * moves exact bytes in the same chunks and resumes the same way, and every binding is the wallet's.
 */
class IrohaPeerNfcSpartanExchangeV1Test {
    private val fixtures = IrohaPeerNfcSpartanFixturesV1
    private val aid = IrohaPeerNfcTestFixturesV1.BPNG_AID
    private val readerLimits = IrohaPeerNfcLimitsV1(maximumReadChunkBytes = 240, maximumWriteChunkBytes = 200)
    private val request = fixtures.request()
    private val payment = fixtures.payment()
    private val status = fixtures.status()

    private fun tap1(reader: IrohaPeerNfcLoopbackV1, wallet: IrohaPeerNfcSpartanMockWalletV1): IrohaPeerNfcRequestReadV1 =
        IrohaPeerNfcReaderExchangeV1.readRequest(aid, wallet.policy, readerLimits, reader)

    private fun tap2(
        reader: IrohaPeerNfcLoopbackV1,
        wallet: IrohaPeerNfcSpartanMockWalletV1,
        checkpoint: ByteArray?,
    ): IrohaPeerNfcReaderExchangeResultV1 = IrohaPeerNfcReaderExchangeV1.run(
        checkpoint,
        aid,
        wallet.policy,
        readerLimits,
        transceive = reader,
        loadOrCreateDurableCheckpoint = IrohaPeerNfcReaderExchangeV1.LoadOrCreateDurableCheckpoint { _, _ ->
            fail("a restored exchange never creates a checkpoint")
        },
        updateDurableCheckpoint = reader.updateCheckpoint,
    )

    private fun error(block: () -> Unit): IrohaPeerNfcErrorV1 = assertFailsWith<IrohaPeerNfcExceptionV1> { block() }.error

    /** A payer that paid [request] with [payment] and whose verification accepts [status]; a receiver that issued it. */
    private fun wallets(): Pair<IrohaPeerNfcSpartanMockWalletV1, IrohaPeerNfcSpartanMockWalletV1> {
        val payer = IrohaPeerNfcSpartanMockWalletV1().also { it.send(request, payment); it.accept(payment, status) }
        val receiver = IrohaPeerNfcSpartanMockWalletV1().also { it.issue(request, status) }
        return payer to receiver
    }

    @Test
    fun `real sized payment and status cross the card in chunks under the wallet bindings`() {
        assertEquals(10_711, payment.byteCount)
        assertEquals(9_400, status.byteCount)
        assertEquals(IrohaPeerContentEncodingV1.NONE, payment.encoding)
        val (payer, receiver) = wallets()
        val card = fixtures.card(receiver, request)
        val reader = IrohaPeerNfcLoopbackV1(card, status.encode())
        payer.journal = reader.log

        val read = tap1(reader, payer)
        assertEquals(request, read.request)
        assertTrue(reader.writeSizes.isEmpty())
        assertEquals(1, payer.count("request"))

        val checkpoint = read.senderCheckpoint(payment.encode(), payer.policy).encode()
        reader.readSizes.clear()
        val result = tap2(reader, payer, checkpoint)
        assertEquals(status, result.acknowledgement)
        assertEquals(IrohaPeerNfcConfirmationStateV1.CONFIRMED, result.confirmationState)
        // The exact Payment and the exact status crossed whole, in the reader's chunk sizes.
        assertTrue(reader.writeSizes.all { it <= 200 } && reader.writeSizes.sum() == payment.byteCount)
        assertTrue(reader.readSizes.all { it <= 240 } && reader.readSizes.sum() >= status.byteCount)
        assertContentEquals(payment.encode(), result.checkpoint.payment.encode())
        assertEquals(1, reader.durableCommits)
        assertEquals(1, reader.admissionRecords.size)
        assertEquals(1, reader.persistedCheckpoints.size)
        assertEquals(IrohaPeerNfcPhaseV1.COMPLETE, card.receiver.phase)
        // The wallet verified the status once, before it was durable and before it was confirmed.
        assertEquals(1, payer.count("status"))
        assertTrue(reader.log.indexOf("verify:status") < reader.log.indexOf("persist:checkpoint"))
        assertTrue(reader.log.indexOf("persist:checkpoint") < reader.log.indexOf("CONFIRM_ACKNOWLEDGEMENT"))
        // The card answered only the status its own wallet retains for this Request.
        assertTrue(receiver.count("issued") >= 1)

        // Restoring the final checkpoint asks the wallet again and observes COMPLETE without another commit.
        val again = tap2(reader, payer, result.checkpoint.encode())
        assertEquals(status, again.acknowledgement)
        assertEquals(1, reader.durableCommits)
        assertEquals(2, payer.count("status"))
    }

    @Test
    fun `nothing is carried on shape alone without the wallet verifier`() {
        val session = ByteArray(16) { 0x21 }
        val shapeOnly = IrohaPeerNfcProfilePolicyV1(fixtures.PROFILE)
        assertTrue(shapeOnly.requiresExchangeVerifier)
        // Every message is structurally valid, and none of these exists without the wallet.
        assertEquals(IrohaPeerNfcErrorV1.CONTINUITY_MISMATCH, error { IrohaPeerNfcReceiverSessionV1(session, request.encode()) })
        assertEquals(IrohaPeerNfcErrorV1.CONTINUITY_MISMATCH,
            error { IrohaPeerNfcSenderCheckpointV1(session, request.encode(), payment.encode()) })
        assertEquals(IrohaPeerNfcErrorV1.CONTINUITY_MISMATCH,
            error { IrohaPeerNfcSenderCheckpointV1(session, request.encode(), payment.encode(), status.encode(), shapeOnly) })

        val (payer, receiver) = wallets()
        val reader = IrohaPeerNfcLoopbackV1(fixtures.card(receiver, request), status.encode())
        assertEquals(IrohaPeerNfcErrorV1.CONTINUITY_MISMATCH,
            error { IrohaPeerNfcReaderExchangeV1.readRequest(aid, shapeOnly, readerLimits, reader) })
        // A Request read with the wallet still needs the wallet's policy to become a checkpoint.
        val read = tap1(reader, payer)
        assertEquals(IrohaPeerNfcErrorV1.CONTINUITY_MISMATCH, error { read.senderCheckpoint(payment.encode()) })
        assertEquals(0, reader.count(IrohaPeerNfcOperationV1.BEGIN_PAYMENT))
    }

    @Test
    fun `a request outside the signed selection is refused before review and never hosted`() {
        val otherScheme = ByteArray(32) { 0x61 }
        val foreign = fixtures.request(scheme = otherScheme)
        val foreignWallet = IrohaPeerNfcSpartanMockWalletV1(scheme = otherScheme)
        val (payer, receiver) = wallets()

        // The payer's wallet selected another scheme than the card's Request names.
        val reader = IrohaPeerNfcLoopbackV1(fixtures.card(foreignWallet, foreign), status.encode())
        assertEquals(IrohaPeerNfcErrorV1.CONTINUITY_MISMATCH, error { tap1(reader, payer) })
        assertEquals(IrohaPeerNfcErrorV1.CONTINUITY_MISMATCH, error {
            IrohaPeerNfcReaderExchangeV1.run(
                null, aid, payer.policy, readerLimits, transceive = reader,
                loadOrCreateDurableCheckpoint = IrohaPeerNfcReaderExchangeV1.LoadOrCreateDurableCheckpoint { _, _ ->
                    fail("no value is created for a Request outside the signed selection")
                },
                updateDurableCheckpoint = reader.updateCheckpoint,
            )
        })
        assertEquals(0, reader.count(IrohaPeerNfcOperationV1.BEGIN_PAYMENT))
        // Another asset is another selection too.
        assertEquals(IrohaPeerNfcErrorV1.CONTINUITY_MISMATCH, error {
            IrohaPeerNfcSenderCheckpointV1(ByteArray(16) { 1 }, fixtures.request(asset = ByteArray(32) { 0x62 }).encode(),
                payment.encode(), null, payer.policy)
        })
        // A wallet never hosts a Request of another selection.
        assertEquals(IrohaPeerNfcErrorV1.CONTINUITY_MISMATCH, error { fixtures.card(receiver, foreign) })
    }

    @Test
    fun `messages of another profile are refused in both directions and wallet V1 is unchanged`() {
        val legacy = IrohaPeerNfcTestFixturesV1
        val (payer, receiver) = wallets()
        val session = ByteArray(16) { 0x31 }

        // Either reader refuses the other profile's card before reading its Request.
        val legacyCard = IrohaPeerNfcLoopbackV1(legacy.card())
        assertEquals(IrohaPeerNfcErrorV1.INVALID_PROFILE, error { tap1(legacyCard, payer) })
        val spartanCard = IrohaPeerNfcLoopbackV1(fixtures.card(receiver, request), status.encode())
        assertEquals(IrohaPeerNfcErrorV1.INVALID_PROFILE,
            error { IrohaPeerNfcReaderExchangeV1.readRequest(aid, legacy.POLICY, readerLimits, spartanCard) })
        assertEquals(0, spartanCard.count(IrohaPeerNfcOperationV1.READ_REQUEST))

        // No checkpoint mixes profiles, whichever message is the foreign one.
        assertEquals(IrohaPeerNfcErrorV1.INVALID_PROFILE,
            error { IrohaPeerNfcSenderCheckpointV1(session, request.encode(), legacy.payment.encode(), null, payer.policy) })
        assertEquals(IrohaPeerNfcErrorV1.INVALID_PROFILE,
            error { IrohaPeerNfcSenderCheckpointV1(session, request.encode(), payment.encode(), legacy.credited.encode(), payer.policy) })
        assertEquals(IrohaPeerNfcErrorV1.INVALID_PROFILE,
            error { IrohaPeerNfcSenderCheckpointV1(session, legacy.request.encode(), payment.encode()) })
        assertEquals(IrohaPeerNfcErrorV1.INVALID_PROFILE,
            error { IrohaPeerNfcSenderCheckpointV1(session, legacy.request.encode(), legacy.payment.encode(), null, payer.policy) })
        // A card under one profile refuses the other profile's Payment header.
        val card = fixtures.card(receiver, request, session)
        val begin = IrohaPeerNfcCommandV1.BeginPayment(session, request.canonicalHash,
            legacy.payment.encode().copyOfRange(0, IrohaPeerWireMessageV1.HEADER_LENGTH))
        assertEquals(IrohaPeerNfcErrorV1.INVALID_PROFILE, error { card.receiver.preparePaymentAdmission(begin) })

        // The wallet V1 profile is still bound by reading its messages: a verifier changes nothing and is not asked.
        val bystander = IrohaPeerNfcSpartanMockWalletV1()
        val legacyPolicy = IrohaPeerNfcProfilePolicyV1(legacy.PROFILE, bystander)
        assertTrue(!legacyPolicy.requiresExchangeVerifier)
        assertEquals(legacy.POLICY, legacyPolicy)
        val legacyReader = IrohaPeerNfcLoopbackV1(legacy.card())
        val read = IrohaPeerNfcReaderExchangeV1.readRequest(aid, legacyPolicy, readerLimits, legacyReader)
        val result = IrohaPeerNfcReaderExchangeV1.run(
            read.senderCheckpoint(legacy.payment.encode(), legacyPolicy).encode(), aid, legacyPolicy, readerLimits,
            transceive = legacyReader,
            loadOrCreateDurableCheckpoint = IrohaPeerNfcReaderExchangeV1.LoadOrCreateDurableCheckpoint { _, _ -> fail("restored") },
            updateDurableCheckpoint = legacyReader.updateCheckpoint,
        )
        assertEquals(legacy.credited, result.acknowledgement)
        assertTrue(bystander.calls.isEmpty())
    }

    @Test
    fun `a swapped request another payment or an unverified status never becomes a checkpoint`() {
        val session = ByteArray(16) { 0x41 }
        val (payer, receiver) = wallets()
        val otherRequest = fixtures.request(nonce = 5)
        val otherPayment = fixtures.payment(seed = 11)
        val otherStatus = fixtures.status(seed = 13)

        // Request swap: the wallet made no Send for the other Request, so its Payment binds nothing there.
        assertEquals(IrohaPeerNfcErrorV1.CONTINUITY_MISMATCH,
            error { IrohaPeerNfcSenderCheckpointV1(session, otherRequest.encode(), payment.encode(), null, payer.policy) })
        // Payment swap: only the exact retained bytes are the Payment of this Request.
        assertEquals(IrohaPeerNfcErrorV1.CONTINUITY_MISMATCH,
            error { IrohaPeerNfcSenderCheckpointV1(session, request.encode(), otherPayment.encode(), null, payer.policy) })
        // A status the wallet's verification refuses is never part of a checkpoint.
        assertEquals(IrohaPeerNfcErrorV1.CONTINUITY_MISMATCH, error {
            IrohaPeerNfcSenderCheckpointV1(session, request.encode(), payment.encode(), otherStatus.encode(), payer.policy)
        })

        // A card that now hosts another Request is another peer: no Payment byte is written to it.
        val checkpoint = IrohaPeerNfcSenderCheckpointV1(session, request.encode(), payment.encode(), null, payer.policy).encode()
        receiver.issue(otherRequest, otherStatus)
        val swapped = IrohaPeerNfcLoopbackV1(fixtures.card(receiver, otherRequest, session), otherStatus.encode())
        assertEquals(IrohaPeerNfcErrorV1.CONTINUITY_MISMATCH, error { tap2(swapped, payer, checkpoint) })
        assertEquals(0, swapped.count(IrohaPeerNfcOperationV1.BEGIN_PAYMENT))
        assertEquals(0, swapped.count(IrohaPeerNfcOperationV1.WRITE))
    }

    @Test
    fun `a status the payer wallet refuses is neither persisted nor confirmed`() {
        val session = ByteArray(16) { 0x43 }
        val forged = fixtures.status(seed = 29)
        // The receiver's wallet answers a status; the payer's wallet verification accepts only another one.
        val payer = IrohaPeerNfcSpartanMockWalletV1().also { it.send(request, payment); it.accept(payment, status) }
        val receiver = IrohaPeerNfcSpartanMockWalletV1().also { it.issue(request, forged) }
        val card = fixtures.card(receiver, request, session)
        val reader = IrohaPeerNfcLoopbackV1(card, forged.encode())
        payer.journal = reader.log
        val checkpoint = tap1(reader, payer).senderCheckpoint(payment.encode(), payer.policy).encode()

        assertEquals(IrohaPeerNfcErrorV1.CONTINUITY_MISMATCH, error { tap2(reader, payer, checkpoint) })
        assertTrue("verify:status" in reader.log)
        assertTrue(reader.persistedCheckpoints.isEmpty())
        assertEquals(0, reader.count(IrohaPeerNfcOperationV1.CONFIRM_ACKNOWLEDGEMENT))
        assertEquals(IrohaPeerNfcPhaseV1.ACKNOWLEDGEMENT_READY, card.receiver.phase)
        // The same checkpoint tried again reads the same status and is refused again, with no second commit.
        assertEquals(IrohaPeerNfcErrorV1.CONTINUITY_MISMATCH, error { tap2(reader, payer, checkpoint) })
        assertTrue(reader.persistedCheckpoints.isEmpty())
        assertEquals(1, reader.durableCommits)
    }

    @Test
    fun `a card never answers a status its own wallet does not retain`() {
        val session = ByteArray(16) { 0x45 }
        val (payer, receiver) = wallets()
        val card = fixtures.card(receiver, request, session)
        // The application behind the card answers another status than the receiver's wallet retains.
        val reader = IrohaPeerNfcLoopbackV1(card, fixtures.status(seed = 31).encode())
        val checkpoint = tap1(reader, payer).senderCheckpoint(payment.encode(), payer.policy).encode()
        val refused = assertFailsWith<IrohaPeerNfcExceptionV1> { tap2(reader, payer, checkpoint) }
        assertEquals(IrohaPeerNfcErrorV1.PEER_PERSISTENCE_FAILURE, refused.error)
        assertEquals(IrohaPeerNfcOperationV1.COMMIT, refused.operation)
        assertTrue(reader.acknowledgementRecords.isEmpty())
        assertEquals(IrohaPeerNfcPhaseV1.PAYMENT_RECEIVING, card.receiver.phase)
        assertEquals(0, payer.count("status"))
    }

    @Test
    fun `every value phase resumes with the exact retained payment after a lost response`() {
        for (lost in listOf(
            IrohaPeerNfcOperationV1.BEGIN_PAYMENT,
            IrohaPeerNfcOperationV1.WRITE,
            IrohaPeerNfcOperationV1.COMMIT,
            IrohaPeerNfcOperationV1.READ_ACKNOWLEDGEMENT,
            IrohaPeerNfcOperationV1.CONFIRM_ACKNOWLEDGEMENT,
        )) {
            val (payer, receiver) = wallets()
            val card = fixtures.card(receiver, request)
            val reader = IrohaPeerNfcLoopbackV1(card, status.encode())
            val initial = tap1(reader, payer).senderCheckpoint(payment.encode(), payer.policy).encode()
            reader.loseResponseFor = lost
            if (lost == IrohaPeerNfcOperationV1.CONFIRM_ACKNOWLEDGEMENT) {
                val result = tap2(reader, payer, initial)
                assertEquals(IrohaPeerNfcConfirmationStateV1.RESPONSE_UNKNOWN, result.confirmationState, lost.name)
                assertEquals(status, result.acknowledgement, lost.name)
                assertEquals(1, payer.count("status"), lost.name)
                continue
            }
            assertFailsWith<IrohaPeerNfcLostResponseV1>(lost.name) { tap2(reader, payer, initial) }
            // Only the durable checkpoint and the card's status decide the resume: the same Payment bytes.
            val retry = reader.persistedCheckpoints.lastOrNull() ?: initial
            val result = tap2(reader, payer, retry)
            assertEquals(status, result.acknowledgement, lost.name)
            assertContentEquals(payment.encode(), result.checkpoint.payment.encode(), lost.name)
            assertEquals(1, reader.durableCommits, lost.name)
            assertEquals(1, reader.admissionRecords.size, lost.name)
            assertEquals(1, reader.persistedCheckpoints.size, lost.name)
            assertEquals(IrohaPeerNfcPhaseV1.COMPLETE, card.receiver.phase, lost.name)
            // One wallet verification of the one status, however many times the radio was lost.
            assertEquals(1, payer.count("status"), lost.name)
        }
    }

    @Test
    fun `a receiver restored from its durable records completes the same exchange only with its wallet`() {
        val session = ByteArray(16) { 0x47 }
        val (payer, receiver) = wallets()
        val requestBytes = request.encode()
        val checkpoint = IrohaPeerNfcSenderCheckpointV1(session, requestBytes, payment.encode(), null, payer.policy).encode()

        val first = IrohaPeerNfcReceiverSessionV1(session, requestBytes, profilePolicy = receiver.policy)
        val reader = IrohaPeerNfcLoopbackV1(IrohaPeerNfcCardEndpointV1.direct(aid, first), status.encode())
        reader.loseResponseFor = IrohaPeerNfcOperationV1.READ_ACKNOWLEDGEMENT
        assertFailsWith<IrohaPeerNfcLostResponseV1> { tap2(reader, payer, checkpoint) }
        assertEquals(IrohaPeerNfcPhaseV1.ACKNOWLEDGEMENT_READY, first.phase)

        // Relaunch from IPA1 and IDA1: the wallet is asked for its retained status before the card answers it.
        val admission = IrohaPeerNfcDurablePaymentAdmissionV1.decode(reader.admissionRecords.single(), receiver.policy)
        val acknowledgement = IrohaPeerNfcDurableAcknowledgementV1.decode(reader.acknowledgementRecords.single(), receiver.policy)
        val issuedBefore = receiver.count("issued")
        val restored = IrohaPeerNfcReceiverSessionV1(session, requestBytes, acknowledgement, admission, receiver.policy)
        assertTrue(receiver.count("issued") > issuedBefore)
        assertEquals(IrohaPeerNfcPhaseV1.ACKNOWLEDGEMENT_READY, restored.phase)
        reader.endpoint = IrohaPeerNfcCardEndpointV1.direct(aid, restored)
        val result = tap2(reader, payer, checkpoint)
        assertEquals(status, result.acknowledgement)
        assertEquals(1, reader.durableCommits)
        assertEquals(IrohaPeerNfcPhaseV1.COMPLETE, restored.phase)

        // The same records under a wallet that retains no such status restore nothing.
        val amnesiac = IrohaPeerNfcSpartanMockWalletV1()
        assertEquals(IrohaPeerNfcErrorV1.CONTINUITY_MISMATCH,
            error { IrohaPeerNfcReceiverSessionV1(session, requestBytes, acknowledgement, admission, amnesiac.policy) })
    }

    @Test
    fun `a new session of the same request takes only the same retained payment`() {
        val (payer, receiver) = wallets()
        val first = fixtures.card(receiver, request, ByteArray(16) { 0x4a })
        val reader = IrohaPeerNfcLoopbackV1(first, status.encode())
        val old = tap1(reader, payer).senderCheckpoint(payment.encode(), payer.policy).encode()
        reader.loseResponseFor = IrohaPeerNfcOperationV1.WRITE
        assertFailsWith<IrohaPeerNfcLostResponseV1> { tap2(reader, payer, old) }
        assertEquals(IrohaPeerNfcPhaseV1.PAYMENT_RECEIVING, first.receiver.phase)

        // The receiver armed its card again: the same Request under a new session, nothing received there.
        val second = fixtures.card(receiver, request, ByteArray(16) { 0x4b })
        reader.endpoint = second
        // The saved checkpoint names the old session, so this card is another peer for it and receives nothing.
        val begun = reader.count(IrohaPeerNfcOperationV1.BEGIN_PAYMENT)
        assertEquals(IrohaPeerNfcErrorV1.CONTINUITY_MISMATCH, error { tap2(reader, payer, old) })
        assertEquals(begun, reader.count(IrohaPeerNfcOperationV1.BEGIN_PAYMENT))
        assertEquals(IrohaPeerNfcPhaseV1.REQUEST_READY, second.receiver.phase)

        // A checkpoint for the new session exists only for the Payment the wallet already retains for this
        // Request: redelivery is a replay, and no other Payment can take its place.
        val read = tap1(reader, payer)
        assertEquals(request, read.request)
        assertEquals(IrohaPeerNfcErrorV1.CONTINUITY_MISMATCH,
            error { read.senderCheckpoint(fixtures.payment(seed = 11).encode(), payer.policy) })
        val result = tap2(reader, payer, read.senderCheckpoint(payment.encode(), payer.policy).encode())
        assertEquals(status, result.acknowledgement)
        assertContentEquals(payment.encode(), result.checkpoint.payment.encode())
        assertEquals(IrohaPeerNfcPhaseV1.COMPLETE, second.receiver.phase)
        assertEquals(1, reader.durableCommits)
        assertEquals(1, payer.count("status"))
    }

    @Test
    fun `a payment is bounded at 12182 bytes before any byte is written`() {
        assertEquals(12_182, IrohaPeerNfcV1.MAXIMUM_PAYMENT_BYTES)
        val largest = fixtures.payment(messageBytes = 12_182)
        assertEquals(12_182, largest.byteCount)
        val payer = IrohaPeerNfcSpartanMockWalletV1().also { it.send(request, largest) }
        val session = ByteArray(16) { 0x49 }
        val checkpoint = IrohaPeerNfcSenderCheckpointV1(session, request.encode(), largest.encode(), null, payer.policy)
        assertEquals(12_182, checkpoint.payment.byteCount)
        assertEquals(12_182,
            IrohaPeerNfcPaymentDescriptorV1(largest.encode().copyOfRange(0, IrohaPeerWireMessageV1.HEADER_LENGTH)).messageLength)
        // One byte more is not a Payment of this profile at all, so it reaches no checkpoint and no card.
        assertFailsWith<IllegalArgumentException> { fixtures.payment(messageBytes = 12_183) }
        val oversized = largest.encode() + byteArrayOf(0)
        assertFailsWith<IrohaPeerNfcExceptionV1> {
            IrohaPeerNfcSenderCheckpointV1(session, request.encode(), oversized, null, payer.policy)
        }
        // The largest CreditStatus of the profile is 10,084 bytes on the card.
        assertEquals(10_084, fixtures.status(messageBytes = 10_084).byteCount)
        assertFailsWith<IllegalArgumentException> { fixtures.status(messageBytes = 10_085) }
    }
}
