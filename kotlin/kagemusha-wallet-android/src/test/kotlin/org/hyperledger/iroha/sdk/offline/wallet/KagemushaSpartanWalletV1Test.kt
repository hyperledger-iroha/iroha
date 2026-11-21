package org.hyperledger.iroha.sdk.offline.wallet

import org.hyperledger.iroha.sdk.offline.*
import java.io.File
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.test.*
import org.junit.jupiter.api.Test

class KagemushaSpartanWalletV1Test {
    @Test fun unloadSettlementDistinguishesPayoutRetryAndMissingEpoch() {
        fun parse(bytes: ByteArray) = KagemushaSpartanUnloadSettlementV1(
            KagemushaSpartanReplyV1(102, -1, 0, 0, 0, 0, bytes))
        val wire = ByteArray(83)
        wire[0] = 17
        wire[32] = 8
        wire[74] = 1
        wire[75] = 2
        assertEquals(2.toBigInteger(), parse(wire).requiredEpoch)
        wire[74] = 0
        assertFailsWith<KagemushaWalletExceptionV1> { parse(wire) }
        wire[75] = 0
        wire[32] = 9
        wire[40] = 1
        wire[73] = 1
        val retry = parse(wire)
        assertFalse(retry.accepted)
        assertTrue(retry.paid)
        assertEquals(9.toBigInteger(), retry.height)
        wire[72] = 1
        assertTrue(parse(wire).accepted)
        wire[73] = 0
        assertFailsWith<KagemushaWalletExceptionV1> { parse(wire) }
    }

    @Test fun `local blacklist installation requires its exact native digest`() {
        val digest = ByteArray(32) { 7 }
        fun reply(status: Int, token: Long, bytes: ByteArray) =
            KagemushaSpartanReplyV1(status, -1, 0, token, 0, 0, bytes)
        val valid = reply(95, 0, digest)
        assertContentEquals(digest, valid.blacklistDigest())
        valid.blacklistDigest().fill(0)
        assertContentEquals(digest, valid.blacklistDigest())
        val invalid = listOf(reply(94, 0, digest), reply(95, 1, digest),
            reply(95, 0, byteArrayOf()), reply(95, 0, ByteArray(32)))
        for (bad in invalid) {
            assertFailsWith<KagemushaWalletExceptionV1> { bad.blacklistDigest() }
        }
    }

    @Test fun `signed asset scale renders exact unsigned base units`() {
        fun value(atoms: String, scale: Int) = spartanScaledAmount(java.math.BigInteger(atoms), scale)
        assertEquals("0.00", value("0", 2))
        assertEquals("0.01", value("1", 2))
        assertEquals("1.00", value("100", 2))
        val maximum = "340282366920938463463374607431768211455"
        assertEquals(maximum, value(maximum, 0))
        assertEquals("34028236692.0938463463374607431768211455", value(maximum, 28))
    }

    @Test fun `wallet owned command framing and bounded selected snapshot`() {
        val id = ByteArray(32) { 4 }
        val face = KagemushaWalletUInt128V1(1, 2)
        val empty = byteArrayOf()
        val base = KagemushaSpartanWalletV1.unloadCommand(id, face, empty, empty, empty)
        assertEquals(61, base.size)
        assertContentEquals(byteArrayOf(75,83,79,49,6,32,0,0,0), base.copyOfRange(0,9))
        assertContentEquals(byteArrayOf(1,0,0,0,0,0,0,0,2,0,0,0,0,0,0,0), base.copyOfRange(45,61))
        assertContentEquals(base + byteArrayOf(1,0,0,0,7,1,0,0,0,8,1,0,0,0,9),
            KagemushaSpartanWalletV1.unloadCommand(id, face, byteArrayOf(7), byteArrayOf(8), byteArrayOf(9)))
        kotlin.test.assertFailsWith<IllegalArgumentException> {
            KagemushaSpartanWalletV1.unloadCommand(id, face, byteArrayOf(7), empty, empty)
        }
        assertContentEquals(byteArrayOf(75,83,79,49,1), KagemushaSpartanWalletV1.ownedCommand(KagemushaSpartanOperationV1.BOOTSTRAP))
        assertContentEquals(byteArrayOf(75,83,79,49,2,1,0,0,0,7,2,0,0,0,8,9),
            KagemushaSpartanWalletV1.ownedCommand(KagemushaSpartanOperationV1.LOAD, byteArrayOf(7),byteArrayOf(8,9)))
        fun read(b: ByteArray) = KagemushaSpartanSnapshotV1(KagemushaSpartanReplyV1(101,-1,0,0,0,0,b))
        val b=ByteArray(196); b[0]=1; b[2]=3; b[3]=28
        for(i in listOf(4,36,68,100,132,156,164)) b[i]=1
        b.fill(0xff.toByte(),180,188)
        val snapshot=read(b)
        val released=assertIs<KagemushaSpartanSnapshotV1.Stage.Released>(snapshot.stage)
        assertEquals(-1L,released.spendable.low); assertEquals(0L,released.spendable.high)
        assertEquals(1L,released.balance.high); assertEquals(1L,released.burned.low)
        snapshot.walletId().fill(0); assertEquals(1,snapshot.walletId()[0].toInt())
        for(i in listOf(0,2,4,36,68,100,156,164,180)) {
            val bad=b.copyOf(); bad[i]=(bad[i].toInt() xor 1).toByte()
            assertFailsWith<KagemushaWalletExceptionV1> { read(bad) }
        }
        val pending=b.copyOf(100).also { it[2]=2 }
        assertIs<KagemushaSpartanSnapshotV1.Stage.Pending>(read(pending).stage)
        assertIs<KagemushaSpartanSnapshotV1.Stage.Enrolled>(read(b.copyOf(68).also {it[2]=1}).stage)
        assertFailsWith<KagemushaWalletExceptionV1> { read(pending + byteArrayOf(0)) }
    }

    @Test fun `signed time anchor refresh uses wallet action seven with bounded originals`() {
        assertContentEquals(byteArrayOf(75,83,79,49,7,1,0,0,0,7,2,0,0,0,8,9),
            KagemushaSpartanWalletV1.refreshTimeAnchorCommand(byteArrayOf(7), byteArrayOf(8,9)))
        val longest = KagemushaSpartanWalletV1.refreshTimeAnchorCommand(ByteArray(512) { 1 }, ByteArray(512) { 2 })
        assertEquals(5 + 4 + 512 + 4 + 512, longest.size)
        for ((anchor, certificate) in listOf(byteArrayOf() to byteArrayOf(8), byteArrayOf(7) to byteArrayOf(),
                ByteArray(513) to byteArrayOf(8), byteArrayOf(7) to ByteArray(513))) {
            assertFailsWith<IllegalArgumentException> {
                KagemushaSpartanWalletV1.refreshTimeAnchorCommand(anchor, certificate)
            }
        }
    }

    @Test fun `signed open Request framing and native reply use the exact terms`() {
        fun payload(fee: Boolean): ByteArray {
            val size = if (fee) 1_677 else 1_250
            val frame = ByteBuffer.allocate(17 + size).order(ByteOrder.BIG_ENDIAN)
                .put("KWRQAC1\u0000".toByteArray()).putInt(size).putInt(1).array()
            frame[frame.lastIndex] = 97
            for ((at,value) in listOf(16 to 1, 258 to 7, 402 to 4, 434 to 9, 498 to 1, 628 to 4, 1094 to 1, 1128 to 1, 1129 to 4)) frame[at] = value.toByte()
            if (fee) for ((at,value) in listOf(274 to 1, 1266 to 1, 1300 to 3, 1301 to 4, 1438 to 1)) frame[at] = value.toByte()
            return frame
        }
        fun message(bytes: ByteArray) = IrohaPeerWireMessageV1(IrohaPeerCanonicalPayload(
            IrohaPeerPayloadProfile.KAGEMUSHA_SPARTAN_PAYMENT_V3, IrohaPeerPayloadKind.REQUEST, 3, bytes))
        for (fee in listOf(false, true)) {
            val bytes = payload(fee)
            val envelope = IrohaPeerSpartanOpenRequestV3(bytes)
            val peer = message(bytes)
            val completed = envelope.request()
            for (at in listOf(66, 129, 194, 209)) completed[at] = 0xff.toByte()
            fun agrees(request: ByteArray) = KagemushaSpartanVerifiedPaymentV1(ByteArray(32) { 1 } + request).matches(envelope)
            assertTrue(agrees(completed))
            for (at in listOf(0, 65, 130, 193, 210, 242, 418, 481)) {
                val altered = completed.copyOf()
                altered[at] = (altered[at].toInt() xor 1).toByte()
                assertFalse(agrees(altered))
            }
            assertContentEquals(byteArrayOf(97), envelope.destinationAccountOriginal())
            assertEquals(if (fee) 344 else 172, envelope.certificates().size)
            assertEquals(if (fee) 255 else 0, envelope.feeSchedule().size)
            val units = ByteArray(16).also { it[0] = 7 }
            val reply = KagemushaSpartanReplyV1(96, -1, 0, 0, 0, 0, peer.encode())
            assertContentEquals(peer.encode(), reply.issuedOpenRequest(units, envelope.feeSchedule()).encode())
            assertFailsWith<KagemushaWalletExceptionV1> { reply.issuedOpenRequest(ByteArray(16), envelope.feeSchedule()) }
            for (at in listOf(0, 11, 15, 16+66, 16+194, 498+66, 1094+34, 16+258)) {
                val bad = bytes.copyOf(); bad[at] = (bad[at].toInt() xor 1).toByte()
                assertFailsWith<IllegalArgumentException> { message(bad) }
            }
            assertFailsWith<IllegalArgumentException> { message(bytes + byteArrayOf(0)) }
            if (!fee) assertEquals("6ec3f326dc20c44ebe71f9cb52d18cae36b35b44946c706e26279fa895910afa", peer.wireHash.joinToString("") { "%02x".format(it) })
            envelope.request().fill(0)
            assertEquals(7, envelope.request()[242].toInt())
        }
    }

    @Test fun `Bootstrap scopes bounds and the authenticated activation result are distinct`() {
        fun reply(kind: Int, b: ByteArray) = KagemushaSpartanReplyV1(kind, -1, 0, 0, 0, 0, b)
        val pins = ByteArray(128) { (it / 32 + 1).toByte() }
        val claim = byteArrayOf(0x4b, 0x47, 4)
        val value = KagemushaSpartanActivationV1(reply(107, pins + claim))
        assertContentEquals(claim, value.original())
        assertContentEquals(ByteArray(32) { 4 }, value.walletId())
        value.original().fill(0)
        assertContentEquals(claim, value.original())
        KagemushaSpartanActivationV1(reply(107, pins + ByteArray(32_768)))
        for (bad in listOf(pins, ByteArray(32) + pins.copyOfRange(32, 128) + claim, pins + ByteArray(32_769))) {
            assertFails { KagemushaSpartanActivationV1(reply(107, bad)) }
        }
        val certified = ByteArray(83).also { it[0] = 1; it[32] = 17; it[40] = 7; it[72] = 1; it[73] = 1 }
        assertTrue(KagemushaSpartanActivationSettlementV1(reply(108, certified)).activated)
        assertFails { KagemushaSpartanActivationSettlementV1(reply(102, certified)) }
        certified[73] = 0
        assertFails { KagemushaSpartanActivationSettlementV1(reply(108, certified)) }
    }

    @Test fun `current Load ordinal preserves both unsigned limbs and rejects an unrelated reply`() {
        val bytes = ByteArray(16).also { it[7] = 0x80.toByte(); it[8] = 7 }
        fun reply(kind: Int, original: ByteArray) = KagemushaSpartanReplyV1(kind, -1, 0, 0, 0, 0, original)
        assertEquals(KagemushaWalletUInt128V1(Long.MIN_VALUE, 7), reply(106, bytes).nextLoadOrdinal())
        assertEquals(KagemushaWalletUInt128V1(0, 0), reply(106, ByteArray(16)).nextLoadOrdinal())
        assertFails { reply(101, bytes).nextLoadOrdinal() }
        assertFails { reply(106, bytes + 0.toByte()).nextLoadOrdinal() }
    }

    @Test fun `CreditStatus transport checks canonical fields and preserves the opaque proof`() {
        fun wrap(bytes: ByteArray) = IrohaPeerWireMessageV1(IrohaPeerCanonicalPayload(
            IrohaPeerPayloadProfile.KAGEMUSHA_SPARTAN_PAYMENT_V3, IrohaPeerPayloadKind.CREDITED, 3, bytes))
        val bytes = "KGSCST3\u0000".toByteArray() + ByteArray(1 + 9 + 160 + 6 + 8) + byteArrayOf(7)
        for (flag in 0..1) {
            bytes[8] = flag.toByte()
            val message = wrap(bytes)
            fun reply(code: Int, original: ByteArray) = KagemushaSpartanReplyV1(code, -1, 0, 0, 0, 0, original)
            assertContentEquals(message.encode(), reply(104, message.encode()).issuedCreditStatus().encode())
            assertEquals(if (flag == 0) KagemushaSpartanCreditOutcomeV1.CREDITED else KagemushaSpartanCreditOutcomeV1.BURNED,
                reply(105, byteArrayOf((1 - flag).toByte())).verifiedCreditOutcome(message))
            assertFails { reply(105, byteArrayOf(flag.toByte())).verifiedCreditOutcome(message) }
            assertFails { reply(104, byteArrayOf((1 - flag).toByte())).verifiedCreditOutcome(message) }
            assertFails { reply(105, byteArrayOf(2)).verifiedCreditOutcome(message) }
            assertFails { reply(104, bytes).issuedCreditStatus() }
            assertContentEquals(bytes, IrohaPeerWireMessageV1.decode(message.encode()).canonicalPayload.bytes)
            assertFails { IrohaPeerKagemushaWalletAdapterV1.decode(message) }
        }
        for ((at, value) in listOf(0 to 0, 8 to 2, 9 to 33, 9 to 1, 188 to 1)) {
            val bad = bytes.copyOf()
            bad[at] = value.toByte()
            assertFailsWith<IllegalArgumentException> { wrap(bad) }
        }
        assertFailsWith<IllegalArgumentException> { wrap(bytes.copyOf(bytes.size - 1)) }
        assertFailsWith<IllegalArgumentException> { wrap(bytes.copyOf(10_001)) }
    }

    @Test fun `receiver status for an incoming Payment hands Native its complete exact original`() {
        val profile = IrohaPeerPayloadProfile.KAGEMUSHA_SPARTAN_PAYMENT_V3
        fun wrap(kind: IrohaPeerPayloadKind, bytes: ByteArray) = IrohaPeerWireMessageV1(
            IrohaPeerCanonicalPayload(profile, kind, 3, bytes), IrohaPeerWireCompressionPolicyV1.PEER_OPTIMIZED)
        fun carrier(envelope: Int, companion: Int, seed: Int): ByteArray =
            ByteBuffer.allocate(16 + envelope + companion).order(ByteOrder.BIG_ENDIAN)
                .put("KWPYPC1\u0000".toByteArray()).putInt(envelope).putInt(companion).array()
                .also { for (at in 16 until it.size) it[at] = (at * 31 + seed).toByte() }
        fun original(request: ByteArray, receive: ByteArray, payment: IrohaPeerWireMessageV1) =
            KagemushaSpartanWalletV1.matchingPaymentOriginal(request, receive, payment)
        val request = ByteArray(32) { 0x21 }
        val receive = ByteArray(32) { 0x44 }
        val canonical = carrier(600, 3, seed = 3)
        val payment = wrap(IrohaPeerPayloadKind.PAYMENT, canonical)

        // The complete IPM1 message: its 84-byte header and the exact canonical Payment, never another encoding.
        val exact = original(request, receive, payment)
        assertContentEquals(payment.encode(), exact)
        assertEquals(IrohaPeerWireMessageV1.HEADER_LENGTH + canonical.size, exact.size)
        assertContentEquals(canonical, exact.copyOfRange(IrohaPeerWireMessageV1.HEADER_LENGTH, exact.size))
        exact.fill(0)
        assertContentEquals(payment.encode(), original(request, receive, payment))
        // Another Payment of the same size is other bytes: Native's comparison with the released Receive tells them apart.
        val other = wrap(IrohaPeerPayloadKind.PAYMENT, carrier(600, 3, seed = 11))
        assertEquals(payment.byteCount, other.byteCount)
        assertFalse(payment.encode().contentEquals(original(request, receive, other)))

        // The bound is the carriers' typed Payment bound, the complete message included: 12,182 bytes.
        assertEquals(12_182, IrohaPeerNfcV1.MAXIMUM_PAYMENT_BYTES)
        val largest = wrap(IrohaPeerPayloadKind.PAYMENT, carrier(10_034, 2_048, seed = 5))
        assertEquals(IrohaPeerNfcV1.MAXIMUM_PAYMENT_BYTES, largest.byteCount)
        assertEquals(IrohaPeerNfcV1.MAXIMUM_PAYMENT_BYTES, original(request, receive, largest).size)
        // No longer Payment exists as a message, so none can be offered for comparison.
        assertFailsWith<IllegalArgumentException> { wrap(IrohaPeerPayloadKind.PAYMENT, carrier(10_035, 2_048, seed = 5)) }
        assertFailsWith<IllegalArgumentException> { wrap(IrohaPeerPayloadKind.PAYMENT, carrier(10_034, 2_049, seed = 5)) }

        // Both identities are 32 nonzero bytes, checked before Native is asked.
        for (bad in listOf(ByteArray(32), ByteArray(31) { 1 }, ByteArray(33) { 1 }, byteArrayOf())) {
            assertFailsWith<IllegalArgumentException> { original(bad, receive, payment) }
            assertFailsWith<IllegalArgumentException> { original(request, bad, payment) }
        }
        // Only a Payment is compared: a Request or a status of the same profile is refused.
        val status = wrap(IrohaPeerPayloadKind.CREDITED, "KGSCST3\u0000".toByteArray() + ByteArray(1 + 9 + 160 + 6 + 8) + byteArrayOf(7))
        val hosted = ByteBuffer.allocate(17 + 1_250).order(ByteOrder.BIG_ENDIAN)
            .put("KWRQAC1\u0000".toByteArray()).putInt(1_250).putInt(1).array().also { frame ->
                frame[frame.lastIndex] = 97
                for ((at, value) in listOf(16 to 1, 258 to 7, 402 to 4, 434 to 9, 498 to 1, 628 to 4, 1094 to 1, 1128 to 1, 1129 to 4)) frame[at] = value.toByte()
            }
        assertFailsWith<IllegalArgumentException> { original(request, receive, status) }
        assertFailsWith<IllegalArgumentException> { original(request, receive, wrap(IrohaPeerPayloadKind.REQUEST, hosted)) }

        // Native answers the ordinary receiver status: the same reply as the archive read, its outcome in the flag.
        fun reply(code: Int, bytes: ByteArray) = KagemushaSpartanReplyV1(code, -1, 0, 0, 0, 0, bytes)
        for (flag in 0..1) {
            val issued = wrap(IrohaPeerPayloadKind.CREDITED,
                "KGSCST3\u0000".toByteArray() + byteArrayOf(flag.toByte()) + ByteArray(9 + 160 + 6 + 8) + byteArrayOf(7))
            val decided = KagemushaSpartanReceiverCreditStatusV1(reply(104, issued.encode()))
            assertContentEquals(issued.encode(), decided.message.encode())
            assertEquals(if (flag == 0) KagemushaSpartanCreditOutcomeV1.CREDITED else KagemushaSpartanCreditOutcomeV1.BURNED,
                decided.outcome)
            // Neither the payer's verification reply nor the compared Payment echoed back is a receiver status.
            assertFailsWith<KagemushaWalletExceptionV1> { KagemushaSpartanReceiverCreditStatusV1(reply(105, issued.encode())) }
            assertFailsWith<KagemushaWalletExceptionV1> { KagemushaSpartanReceiverCreditStatusV1(reply(104, payment.encode())) }
        }
    }

    @Test fun `Spartan QR carries the exact uncompressed Payment without running the installed decoder`() {
        val wrapper = "KWPYPC1\u0000".toByteArray() + byteArrayOf(0, 0, 2, 88, 0, 0, 0, 3) + ByteArray(600) + "abc".toByteArray()
        val profile = IrohaPeerPayloadProfile.KAGEMUSHA_SPARTAN_PAYMENT_V3
        fun message(bytes: ByteArray) = IrohaPeerWireMessageV1(IrohaPeerCanonicalPayload(profile, IrohaPeerPayloadKind.PAYMENT, 3, bytes),
            IrohaPeerWireCompressionPolicyV1.PEER_OPTIMIZED)
        val payment = message(wrapper)
        assertEquals(IrohaPeerContentEncodingV1.NONE, payment.encoding)
        assertEquals("0c03088d2c5cb1d25144cbed3f175633c7d3f40d2d66018e09cd126c6b21647d", payment.wireHash.joinToString("") { "%02x".format(it) })
        assertFails { IrohaPeerKagemushaWalletAdapterV1.decode(payment) }
        assertFails { IrohaPeerWireMessageV1.decode(payment.encode(), IrohaPeerPayloadProfile.KAGEMUSHA_WALLET_V1) }
        val scan = IrohaPeerQRScanSessionV1(profile, IrohaPeerPayloadKind.PAYMENT, 3)
        val received = IrohaPeerQRCodecV1.animatedFrameTexts(payment).firstNotNullOf { scan.ingest(it).message }
        assertContentEquals(payment.encode(), received.encode())
        for (offset in listOf(5, 8, 10, 16, 52, 84, 92, 96)) {
            val bad = payment.encode()
            bad[offset] = (bad[offset].toInt() xor 1).toByte()
            assertFails { IrohaPeerWireMessageV1.decode(bad) }
        }
        for (offset in listOf(0, 8, 12)) {
            val bad = wrapper.copyOf()
            bad[offset] = (bad[offset].toInt() xor 1).toByte()
            assertFails { message(bad) }
        }
    }

    @Test
    fun confirmationKeyRequiresPerUseAuthenticationAcrossKeystoreVersions() {
        for (api in listOf(26, 29, 30, 31, 35)) {
            assertTrue(spartanPerUseValidityV1(api, -1))
            assertEquals(api >= 30, spartanPerUseValidityV1(api, 0))
            for (seconds in listOf(-2, 1, 30, Int.MAX_VALUE)) {
                assertFalse(spartanPerUseValidityV1(api, seconds))
            }
        }
    }

    @Test fun `older phones lacking a strong enrolled biometric get a debit-only refusal`() {
        val absent = androidx.biometric.BiometricManager.BIOMETRIC_ERROR_NONE_ENROLLED
        for (api in 26..29) {
            val refusal = assertFailsWith<KagemushaSpartanDebitUnavailableV1> { requireSpartanAuthenticatorV1(api, absent) }
            assertEquals(KagemushaSpartanDebitUnavailableReasonV1.STRONG_BIOMETRIC_REQUIRED_ON_ANDROID_26_TO_29, refusal.reason)
            assertEquals(androidx.biometric.BiometricManager.Authenticators.BIOMETRIC_STRONG, spartanAuthenticatorsV1(api))
        }
        assertEquals(KagemushaSpartanDebitUnavailableReasonV1.DEVICE_AUTHENTICATION_UNAVAILABLE,
            assertFailsWith<KagemushaSpartanDebitUnavailableV1> { requireSpartanAuthenticatorV1(30, absent) }.reason)
        requireSpartanAuthenticatorV1(26, androidx.biometric.BiometricManager.BIOMETRIC_SUCCESS)
        assertTrue(spartanAuthenticatorsV1(30) and androidx.biometric.BiometricManager.Authenticators.DEVICE_CREDENTIAL != 0)
    }

    @Test fun `shared Send review displays the projected amounts and both complete recipient IDs`() {
        val original = ByteArray(291)
        original[0] = 1
        original[2] = 3
        val wire = java.nio.ByteBuffer.wrap(original).order(java.nio.ByteOrder.LITTLE_ENDIAN)
        wire.putLong(131, 123)
        wire.putLong(147, 7)
        original.fill(0x3a, 163, 195)
        original.fill(0x5c, 195, 227)
        val terms = KagemushaSpartanReviewProjectionV1(original).sendDebitTerms()(2)
        assertTrue(terms.contains("Send 1.23 asset units"))
        assertTrue(terms.contains("Fee  0.07"))
        assertTrue(terms.contains("Total debit  1.30"))
        assertTrue(terms.contains("Receiving wallet ID\n" + "3a".repeat(32)))
        assertTrue(terms.contains("Receiving account ID\n" + "5c".repeat(32)))
        original[2] = 4
        assertFails { KagemushaSpartanReviewProjectionV1(original).sendDebitTerms() }
    }

    @Test fun `debit prompt accepts only its authenticated operation once and cancellation is exact`() {
        val pair = java.security.KeyPairGenerator.getInstance("EC").apply {
            initialize(java.security.spec.ECGenParameterSpec("secp256r1"))
        }.generateKeyPair()
        fun operation() = java.security.Signature.getInstance("SHA256withECDSA").apply { initSign(pair.private) }
        val message = ByteArray(32) { it.toByte() }
        var receipts = 0
        var canceled = 0
        fun prompt(op: java.security.Signature) = KagemushaSpartanDebitPromptV1(op, message, "Reviewed terms",
            { actual, der ->
                assertContentEquals(message, actual)
                assertTrue(java.security.Signature.getInstance("SHA256withECDSA").run {
                    initVerify(pair.public); update(message); verify(der)
                })
                receipts++
            }, { actual -> assertContentEquals(message, actual); canceled++ })
        val op = operation()
        val request = prompt(op)
        assertFails { request.complete(operation()) }
        request.complete(op)
        assertFails { request.complete(op) }
        request.cancel()
        val canceledOp = operation()
        val canceledRequest = prompt(canceledOp)
        canceledRequest.cancel()
        canceledRequest.cancel()
        assertFails { canceledRequest.complete(canceledOp) }
        assertEquals(1, receipts)
        assertEquals(1, canceled)
        val alias = KagemushaSpartanAndroidConfirmationV1.alias(message)
        assertEquals("kgm-spartan-user-presence-v1-" + message.joinToString("") { "%02x".format(it) }, alias)
        assertFails { KagemushaSpartanAndroidConfirmationV1.alias(ByteArray(32)) }
    }

    @Test fun `account admission projects retained outputs only for the authenticated slot`() {
        val parameters = ByteArray(32) { 1 }
        val slot = ByteArray(32) { 2 }
        fun reply(status: Int, token: Long = 3, bytes: ByteArray = parameters + slot) =
            KagemushaSpartanReplyV1(status, -1, 0, token, 0, 0, bytes).checked()
        for (status in listOf(85, 87)) {
            val (id, retainedOutputsOnly) = reply(status).authorizedSlot(3, parameters)
            assertContentEquals(slot, id)
            assertEquals(status == 87, retainedOutputsOnly)
            for (invalid in listOf(reply(status, token = 4), reply(status, bytes = slot + parameters),
                reply(status, bytes = parameters + ByteArray(32)), reply(status, bytes = slot))) {
                assertFailsWith<KagemushaWalletExceptionV1> { invalid.authorizedSlot(3, parameters) }
            }
        }
        assertFailsWith<KagemushaWalletExceptionV1> { reply(86).authorizedSlot(3, parameters) }
    }

    @Test fun `direct time bounds preserve unsigned values and ordering`() {
        val bytes = ByteArray(16)
        bytes[0] = 1
        bytes.fill(-1, 8)
        val interval = KagemushaSpartanTimeIntervalV1(bytes)
        assertEquals(1uL, interval.lowerMs)
        assertEquals(ULong.MAX_VALUE, interval.upperMs)
        assertFailsWith<KagemushaWalletExceptionV1> { KagemushaSpartanTimeIntervalV1(bytes.copyOf(15)) }
        bytes.fill(0, 8)
        assertFailsWith<KagemushaWalletExceptionV1> { KagemushaSpartanTimeIntervalV1(bytes) }
    }

    @Test fun `completed non Send has no Payment but head and completion remain required`() {
        fun reply(status: Int, data: ByteArray = byteArrayOf()) =
            KagemushaSpartanReplyV1(status, -1, 0, 0, 0, 0, data).checked()
        val output = KagemushaSpartanRetainedOutputV1.from(reply(1), KagemushaSpartanOutputV1.PAYMENT)
        assertContentEquals(byteArrayOf(), assertIs<KagemushaSpartanRetainedOutputV1.Released>(output).bytes())
        val claim = ByteArray(16_384) { 3 }
        val retained = KagemushaSpartanRetainedOutputV1.from(reply(1, claim), KagemushaSpartanOutputV1.UNLOAD_CLAIM)
        assertContentEquals(claim, assertIs<KagemushaSpartanRetainedOutputV1.Released>(retained).bytes())
        assertFails { KagemushaSpartanRetainedOutputV1.from(reply(1, claim + byteArrayOf(1)), KagemushaSpartanOutputV1.UNLOAD_CLAIM) }
        for (part in listOf(KagemushaSpartanOutputV1.HEAD, KagemushaSpartanOutputV1.COMPLETION, KagemushaSpartanOutputV1.UNLOAD_CLAIM)) {
            assertFailsWith<KagemushaWalletExceptionV1> { KagemushaSpartanRetainedOutputV1.from(reply(1), part) }
        }
        assertFailsWith<KagemushaWalletExceptionV1> {
            KagemushaSpartanRetainedOutputV1.from(reply(2, byteArrayOf(1)), KagemushaSpartanOutputV1.PAYMENT)
        }
    }
    @Test fun `Unload preview binds both accounts and conserves the face debit`() {
        val vector = generateSequence(File(".").canonicalFile) { it.parentFile }
            .map { File(it, "fixtures/kagemusha/spartan_unload_review_v3.bin") }.first { it.isFile }.readBytes()
        val source = vector.copyOf()
        val review = KagemushaSpartanUnloadProjectionV3(source)
        source.fill(0)
        review.bytes().fill(0)
        assertContentEquals(vector, review.bytes())
        assertEquals(KagemushaWalletUInt128V1(4, 0), review.faceDebit)
        assertEquals(KagemushaWalletUInt128V1(1, 0), review.onlineCharge)
        assertEquals(KagemushaWalletUInt128V1(3, 0), review.netToAccount)
        val account = review.credentialAccountBytes()
        val beneficiary = assertNotNull(review.beneficiaryAccountBytes())
        assertFalse(account.contentEquals(beneficiary))
        assertContentEquals(account, vector.copyOfRange(421, 421 + account.size))
        assertContentEquals(beneficiary, vector.copyOfRange(423 + account.size, vector.size))
        review.credentialAccountBytes().fill(0)
        assertContentEquals(account, review.credentialAccountBytes())
        assertFailsWith<KagemushaWalletExceptionV1> { KagemushaSpartanReviewProjectionV1(vector) }
        assertFailsWith<KagemushaWalletExceptionV1> { KagemushaSpartanTimeAnchorProjectionV2(vector) }

        val free = vector.copyOf(423 + account.size)
        free.fill(0, 421 + account.size, free.size)
        for (range in listOf(227 until 243, 259 until 291, 323 until 355)) free.fill(0, range.first, range.last + 1)
        free[243] = 4
        assertNull(KagemushaSpartanUnloadProjectionV3(free).beneficiaryAccountBytes())
        val borrowed = vector.copyOf()
        borrowed.fill(0, 211, 227); borrowed[219] = 1
        borrowed.fill(-1, 243, 251); borrowed.fill(0, 251, 259)
        assertEquals(KagemushaWalletUInt128V1(-1L, 0), KagemushaSpartanUnloadProjectionV3(borrowed).netToAccount)
        val allCharged = vector.copyOf()
        allCharged[227] = 4; allCharged[243] = 0
        assertEquals(KagemushaWalletUInt128V1(0, 0), KagemushaSpartanUnloadProjectionV3(allCharged).netToAccount)

        for (i in listOf(0, 1, 2, 211, 243, 420)) {
            val invalid = vector.copyOf()
            invalid[i] = (invalid[i].toInt() xor 0x80).toByte()
            assertFailsWith<KagemushaWalletExceptionV1> { KagemushaSpartanUnloadProjectionV3(invalid) }
        }
        for (range in listOf(163 until 195, 211 until 227, 227 until 243, 259 until 291, 323 until 355, 419 until 421)) {
            val invalid = vector.copyOf()
            invalid.fill(0, range.first, range.last + 1)
            assertFailsWith<KagemushaWalletExceptionV1> { KagemushaSpartanUnloadProjectionV3(invalid) }
        }
        for (invalid in listOf(vector.copyOf(vector.size - 1), vector + byteArrayOf(0), ByteArray(16_385))) {
            assertFailsWith<KagemushaWalletExceptionV1> { KagemushaSpartanUnloadProjectionV3(invalid) }
        }
    }

    @Test fun `admission refuses another credential profile before Native`() {
        for (count in listOf(0, 532, 595, 597)) assertFails {
            KagemushaSpartanAccountOriginalsV1(ByteArray(count), ByteArray(172), byteArrayOf(1), byteArrayOf(2))
        }
        for (count in listOf(0, 171, 173)) assertFails {
            KagemushaSpartanAccountOriginalsV1(ByteArray(596), ByteArray(count), byteArrayOf(1), byteArrayOf(2))
        }
        val credential = ByteArray(596) { 7 }
        val originals = KagemushaSpartanAccountOriginalsV1(credential, ByteArray(172), byteArrayOf(1), byteArrayOf(2))
        credential.fill(0); originals.frames()[0].fill(0)
        assertContentEquals(ByteArray(596) { 7 }, originals.frames()[0])
    }
    private fun projection(): ByteArray = ByteArray(291).also {
        it[0] = 1; it[2] = 3
        for ((index, offset) in listOf(3, 35, 67, 99, 163, 195, 227, 259).withIndex()) it.fill((index + 1).toByte(), offset, offset + 32)
        ByteBuffer.wrap(it).order(ByteOrder.LITTLE_ENDIAN).apply {
            putLong(131, -1L); putLong(139, Long.MIN_VALUE); putLong(147, 7); putLong(155, 9)
        }
    }
    @Test fun `review preserves unsigned amounts and every bound identity`() {
        val source = projection()
        val review = KagemushaSpartanReviewProjectionV1(source)
        source.fill(0)
        review.bytes().fill(0)
        assertContentEquals(projection(), review.bytes())
        assertEquals(KagemushaSpartanOperationV1.SEND, review.operation)
        assertEquals(KagemushaWalletUInt128V1(-1L, Long.MIN_VALUE), review.amount)
        assertEquals(KagemushaWalletUInt128V1(7, 9), review.fee)
        val identities = listOf(review.schemeId(), review.walletId(), review.operationId(), review.capsuleDigest(),
            review.receiverWalletId(), review.receiverAccountId(), review.beforeHead(), review.afterHead())
        identities.forEachIndexed { index, bytes -> assertContentEquals(ByteArray(32) { (index + 1).toByte() }, bytes) }
        identities[0].fill(0)
        assertContentEquals(ByteArray(32) { 1 }, review.schemeId())
    }
    @Test fun `review rejects unsupported version kind truncation and trailing data`() {
        val bad = listOf(projection().copyOf(290), projection().copyOf(292),
            projection().also { it[0] = 2 }, projection().also { it[1] = 1 },
            projection().also { it[2] = 0 }, projection().also { it[2] = 5 })
        bad.forEach { assertFailsWith<KagemushaWalletExceptionV1> { KagemushaSpartanReviewProjectionV1(it) } }
    }
    @Test fun `signed time anchor binds unsigned floor and selected heads`() {
        val bytes = ByteArray(243)
        bytes[0] = 2; bytes[2] = 7
        for ((index, offset) in listOf(3, 35, 67, 99, 131, 179, 211).withIndex()) {
            bytes.fill((index + 1).toByte(), offset, offset + 32)
        }
        ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN).apply {
            putLong(163, Long.MIN_VALUE); putLong(171, Long.MIN_VALUE + 1)
        }
        val anchor = KagemushaSpartanTimeAnchorProjectionV2(bytes)
        assertEquals(1uL shl 63, anchor.issuerTime)
        assertEquals((1uL shl 63) + 1uL, anchor.successorFloor)
        val fields = listOf(anchor.schemeId(), anchor.walletId(), anchor.operationId(), anchor.capsuleDigest(),
            anchor.anchorDigest(), anchor.beforeHead(), anchor.afterHead())
        fields.forEachIndexed { index, field -> assertContentEquals(ByteArray(32) { (index + 1).toByte() }, field) }
        anchor.anchorDigest().fill(0)
        assertContentEquals(ByteArray(32) { 5 }, anchor.anchorDigest())
        assertFailsWith<KagemushaWalletExceptionV1> { KagemushaSpartanReviewProjectionV1(bytes) }
        val malformed = listOf(bytes.copyOf(242), bytes.copyOf(244), projection(),
            bytes.copyOf().also { it[0] = 1 }, bytes.copyOf().also { it[1] = 1 },
            bytes.copyOf().also { it[2] = 6 }, bytes.copyOf().also { it[178] = 0 })
        malformed.forEach { assertFailsWith<KagemushaWalletExceptionV1> { KagemushaSpartanTimeAnchorProjectionV2(it) } }
        bytes[171] = 0
        assertEquals(anchor.issuerTime, KagemushaSpartanTimeAnchorProjectionV2(bytes).successorFloor)
    }

    @Test fun `reply domains reject unexpected tokens output shapes and false complete`() {
        fun reply(status: Int, token: Long = 0, data: ByteArray = byteArrayOf()) =
            KagemushaSpartanReplyV1(status, -1, 0, token, 0, 0, data).checked()
        assertFails { reply(82, 0, projection()).requireShape(82, 291, token = true) }
        assertFails { reply(83, 1, ByteArray(514)).requireShape(83, 514) }
        assertFails { reply(81, 1, ByteArray(31)).requireShape(81, 32, token = true) }
        assertFails { KagemushaSpartanExecutionV1(reply(1)) }
        assertFails { KagemushaSpartanExecutionV1(reply(2, data = byteArrayOf(1))) }
        assertFails { KagemushaSpartanExecutionV1(reply(84)) }
        assertEquals(KagemushaSpartanExecutionV1.Status.PENDING, KagemushaSpartanExecutionV1(reply(2)).status)
        val original = byteArrayOf(1, 2, 3)
        val completed = KagemushaSpartanExecutionV1(reply(1, data = original))
        original.fill(0)
        completed.completion()!!.fill(0)
        assertContentEquals(byteArrayOf(1, 2, 3), completed.completion())
    }
    @Test fun `installed open accepts only its own handle and nonzero parameter identity`() {
        val parameters = ByteArray(32) { 7 }
        fun identity(status: Int, token: Long, bytes: ByteArray) =
            KagemushaSpartanRuntimeV1.installedParameterId(KagemushaSpartanReplyV1(status, -1, 0, token, 0, 0, bytes), 9)
        assertContentEquals(parameters, identity(81, 9, parameters))
        for ((status, token, bytes) in listOf(Triple(81, 8L, parameters), Triple(81, 0L, parameters),
            Triple(81, 9L, ByteArray(32)), Triple(81, 9L, ByteArray(31) { 7 }), Triple(15, 9L, parameters))) {
            assertFailsWith<KagemushaWalletExceptionV1> { identity(status, token, bytes) }
        }
    }
    @Test fun `execute validates native sequence and status specific reason`() {
        fun execution(status: Int, low: Long = 0, high: Long = 0, detail: Int = 0): KagemushaSpartanExecutionV1 {
            val bytes = if (status == 1) byteArrayOf(1) else byteArrayOf()
            return KagemushaSpartanExecutionV1(KagemushaSpartanReplyV1(status, -1, 0, low, high, detail, bytes))
        }
        for (status in 1..5) {
            assertFailsWith<KagemushaWalletExceptionV1> { execution(status, low = 1) }
            assertFailsWith<KagemushaWalletExceptionV1> { execution(status, high = 1) }
            if (status != 3) assertFailsWith<KagemushaWalletExceptionV1> { execution(status, detail = 1) }
        }
        for (reason in 0..2) assertEquals(reason, execution(3, detail = reason).detail)
        assertFailsWith<KagemushaWalletExceptionV1> { execution(3, detail = 3) }
        assertFailsWith<KagemushaWalletExceptionV1> { execution(3, detail = -1) }
    }

    @Test fun `verified payment owns its digest and exact 482 byte request`() {
        val bytes = ByteArray(514) { (it % 251).toByte() }
        val result = KagemushaSpartanVerifiedPaymentV1(bytes)
        val digest = bytes.copyOfRange(0, 32); val request = bytes.copyOfRange(32, 514)
        bytes.fill(0); result.digest().fill(0); result.request().fill(0)
        assertContentEquals(digest, result.digest()); assertContentEquals(request, result.request())
        assertFails { KagemushaSpartanVerifiedPaymentV1(ByteArray(513)) }
    }
}
