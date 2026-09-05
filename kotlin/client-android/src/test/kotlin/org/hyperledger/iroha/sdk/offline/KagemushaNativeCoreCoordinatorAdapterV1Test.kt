// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.file.Files
import java.nio.file.Paths
import java.util.EnumSet
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import org.junit.jupiter.api.Test
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter

/** Scripted endpoints test mapping and rejection, never manufacture qualified native evidence. */
class KagemushaNativeCoreCoordinatorAdapterV1Test {
    @Test fun `all ten typed methods map exact fields through native transport`() {
        val f = Fixture()
        val endpoint = Endpoint()
        val core = KagemushaNativeCoreCoordinatorAdapterV1.openEndpoint("/test/store", endpoint)
        endpoint.expect(1, listOf(u32(5), f.id, byteArrayOf(9)), listOf(f.id))
        assertContentEquals(f.id, core.reserveOperationId(5, f.id, byteArrayOf(9)))
        endpoint.expect(2, f.qFields + listOf(f.q.hardwarePolicyDigest()), emptyList())
        core.acceptQualification(f.q, f.q.hardwarePolicyDigest())
        val authenticator = ByteArray(64).also { it[31] = 1; it[63] = 2 }
        endpoint.expect(3, listOf(u32(5), f.id, byteArrayOf(7), byteArrayOf(8), authenticator) + f.qFields, emptyList())
        core.acceptAuthenticatedDeviceReply(5, f.id, byteArrayOf(7), byteArrayOf(8), authenticator, f.q)
        endpoint.expect(4, listOf(f.id, u32(0), f.requestBytes) + f.qFields, listOf(f.id, f.preparationBytes))
        val preparation = core.beginSenderTransition(f.id, f.inputs, f.q)
        endpoint.expect(5, listOf(f.preparationBytes, byteArrayOf(5)), listOf(f.candidateBytes))
        val candidate = core.provePreparedSenderTransition(preparation, byteArrayOf(5))
        endpoint.expect(6, listOf(f.candidateBytes, byteArrayOf(7)), listOf(f.paymentBytes))
        assertContentEquals(f.paymentBytes, core.terminalEnvelope(candidate, byteArrayOf(7)))
        endpoint.expect(7, listOf(f.candidateBytes, f.paymentBytes, byteArrayOf(9), byteArrayOf(10), byteArrayOf(21)),
            listOf(f.paymentBytes, f.aggregateBytes))
        assertContentEquals(f.aggregateBytes, core.acceptInstalledTerminal(candidate, f.paymentBytes,
            byteArrayOf(9), byteArrayOf(10), byteArrayOf(21)).aggregateState())
        endpoint.expect(8, listOf(byteArrayOf(0), f.terminal, u32(0)) + f.qFields,
            listOf(f.id, f.terminal, f.recoveryBytes))
        val recovery = requireNotNull(core.senderRecovery(KagemushaNativeSenderKindV1.PAYMENT, f.terminal, f.q))
        endpoint.expect(8, listOf(byteArrayOf(1), f.id, u32(0)) + f.qFields, emptyList())
        assertNull(core.senderRecoveryByOperationId(KagemushaNativeSenderKindV1.PAYMENT, f.id, f.q))
        endpoint.expect(9, listOf(f.recoveryBytes, byteArrayOf(10)), listOf(f.paymentBytes))
        assertContentEquals(f.paymentBytes, core.recoverTerminalEnvelope(recovery, byteArrayOf(10)))
        endpoint.expect(10, listOf(f.terminal, u32(0), f.requestBytes, f.paymentBytes, u32(0) + f.ackBytes) + f.qFields,
            listOf(f.id, f.preparationBytes, f.envelopeDigest, f.paymentBytes, byteArrayOf(12)))
        val release = core.outboxRelease(f.terminal, f.inputs, f.paymentBytes,
            KagemushaDeviceSenderTerminalReceiptV1.PaymentAcknowledgement(f.ackBytes), f.q)
        assertContentEquals(f.id, release.operationId())
        assertContentEquals(f.envelopeDigest, release.envelopeDigest())
        assertContentEquals(byteArrayOf(12), release.hardwareReleaseAuthorization())
        assertEquals(11, endpoint.calls)
    }

    @Test fun `begin rejects substituted archive operation inputs release credential and Core key`() {
        val f = Fixture()
        val contexts = listOf(f.context(release = digest(97)), f.context(credential = digest(98)), f.context(coreKey = digest(99)))
        val invalid = listOf(
            KagemushaNativeSenderPreparationV1(digest(90), f.context(), f.preparation.inputsDigest()),
            KagemushaNativeSenderPreparationV1(f.id, f.context(), digest(91)),
        ) + contexts.map { KagemushaNativeSenderPreparationV1(f.id, it, f.preparation.inputsDigest()) }
        invalid.forEach { preparation ->
            val core = responding(4, listOf(f.id, KagemushaCoreCoordinatorArchiveV1.encodePreparationShape(preparation)))
            assertFailsWith<IllegalArgumentException> { core.beginSenderTransition(f.id, f.inputs, f.q) }
        }
    }

    @Test fun `redemption maps the full unsigned u128 amount and canonical beneficiary`() {
        val f = Fixture()
        val amount = BigInteger.ONE.shiftLeft(127).add(BigInteger.valueOf(7))
        val inputs = KagemushaDeviceSenderPublicInputsV1.RedeemSplit(amount, f.request.recipient.canonicalPayload())
        val preparation = KagemushaNativeSenderPreparationV1(f.id, f.context(),
            KagemushaCoreCoordinatorArchiveV1.inputsDigestShape(f.id, f.context(), inputs))
        val amountBytes = ByteArray(16).also { it[0] = 7; it[15] = 0x80.toByte() }
        val endpoint = Endpoint().apply {
            expect(4, listOf(f.id, u32(1), amountBytes, f.request.recipient.canonicalPayload()) + f.qFields,
                listOf(f.id, KagemushaCoreCoordinatorArchiveV1.encodePreparationShape(preparation)))
        }
        val core = KagemushaNativeCoreCoordinatorAdapterV1.openEndpoint("/test/store", endpoint)
        assertContentEquals(preparation.inputsDigest(), core.beginSenderTransition(f.id, inputs, f.q).inputsDigest())
        assertEquals(1, endpoint.calls)
    }

    @Test fun `prove rejects a different canonical nested preparation`() {
        val f = Fixture()
        val swapped = KagemushaNativeSenderPreparationV1(digest(99), f.context(), f.preparation.inputsDigest())
        val candidate = KagemushaNativeSenderCandidateV1(swapped, f.candidate.selector, digest(3), byteArrayOf(7))
        val core = responding(5, listOf(KagemushaCoreCoordinatorArchiveV1.encodeCandidateShape(candidate)))
        assertFailsWith<IllegalArgumentException> { core.provePreparedSenderTransition(f.preparation, byteArrayOf(5)) }
    }

    @Test fun `recovery binds both embedded identities and stable lane but permits an older epoch`() {
        val f = Fixture()
        listOf(
            KagemushaNativeSenderRecoveryV1(digest(8), f.terminal, f.context(), f.preparation.inputsDigest()),
            KagemushaNativeSenderRecoveryV1(f.id, digest(8), f.context(), f.preparation.inputsDigest()),
            KagemushaNativeSenderRecoveryV1(f.id, f.terminal, f.context(lane = digest(8)), f.preparation.inputsDigest()),
            KagemushaNativeSenderRecoveryV1(f.id, f.terminal, f.context(generation = BigInteger.valueOf(2)), f.preparation.inputsDigest()),
        ).forEach {
            val core = responding(8, listOf(f.id, f.terminal, KagemushaCoreCoordinatorArchiveV1.encodeRecoveryShape(it)))
            assertFailsWith<IllegalArgumentException> { core.senderRecovery(KagemushaNativeSenderKindV1.PAYMENT, f.terminal, f.q) }
        }
        val core = responding(8, listOf(f.id, f.terminal, f.recoveryBytes))
        val rotated = f.qualification(2)
        assertContentEquals(f.id, requireNotNull(core.senderRecoveryByOperationId(
            KagemushaNativeSenderKindV1.PAYMENT, f.id, rotated)).operationId())
    }

    @Test fun `installed terminal rejects a canonical aggregate from another lane`() {
        val f = Fixture()
        val core = responding(7, listOf(f.paymentBytes, f.aggregate(digest(99))))
        assertFailsWith<IllegalArgumentException> { core.acceptInstalledTerminal(f.candidate, f.paymentBytes,
            byteArrayOf(9), byteArrayOf(10), byteArrayOf(21)) }
    }

    @Test fun `installed terminal rejects a substituted liability pool`() {
        val f = Fixture()
        val pool = f.request.liabilityPoolId()
        val payload = NoritoHeader.decode(f.aggregateBytes, null).payload
        val start = (0..payload.size - pool.size).single { payload.copyOfRange(it, it + pool.size).contentEquals(pool) }
        digest(99).copyInto(payload, start)
        val raw = NoritoCodec.encode(payload, "iroha_data_model::kagemusha::kagemusha_v1::KagemushaAggregateStateCommitmentV1",
            object : TypeAdapter<ByteArray> {
                override fun encode(encoder: NoritoEncoder, value: ByteArray) = encoder.writeBytes(value)
                override fun decode(decoder: NoritoDecoder) = decoder.readBytes(decoder.remaining())
            })
        val padding = f.aggregateBytes.size - NoritoHeader.HEADER_LENGTH - payload.size
        val mutated = raw.copyOfRange(0, NoritoHeader.HEADER_LENGTH) + ByteArray(padding) + raw.copyOfRange(NoritoHeader.HEADER_LENGTH, raw.size)
        val core = responding(7, listOf(f.paymentBytes, mutated))
        assertFailsWith<IllegalArgumentException> { core.acceptInstalledTerminal(f.candidate, f.paymentBytes,
            byteArrayOf(9), byteArrayOf(10), byteArrayOf(21)) }
    }

    @Test fun `release rejects returned operation input and envelope digest substitutions`() {
        val f = Fixture()
        val badInputs = KagemushaNativeSenderPreparationV1(f.id, f.context(), digest(9))
        listOf(
            listOf(digest(8), f.preparationBytes, f.envelopeDigest, f.paymentBytes, byteArrayOf(12)),
            listOf(f.id, KagemushaCoreCoordinatorArchiveV1.encodePreparationShape(badInputs), f.envelopeDigest, f.paymentBytes, byteArrayOf(12)),
            listOf(f.id, f.preparationBytes, digest(8), f.paymentBytes, byteArrayOf(12)),
        ).forEach { response ->
            val core = responding(10, response)
            assertFailsWith<IllegalArgumentException> { core.outboxRelease(f.terminal, f.inputs, f.paymentBytes,
                KagemushaDeviceSenderTerminalReceiptV1.PaymentAcknowledgement(f.ackBytes), f.q) }
        }
    }

    @Test fun `wrong policy credit or receipt kind fails before invoking native`() {
        val f = Fixture()
        val endpoint = Endpoint()
        val core = KagemushaNativeCoreCoordinatorAdapterV1.openEndpoint("/test/store", endpoint)
        assertFailsWith<IllegalArgumentException> { core.acceptQualification(f.q, digest(99)) }
        assertFailsWith<IllegalArgumentException> { core.outboxRelease(digest(99), f.inputs, f.paymentBytes,
            KagemushaDeviceSenderTerminalReceiptV1.PaymentAcknowledgement(f.ackBytes), f.q) }
        val redeem = KagemushaDeviceSenderPublicInputsV1.RedeemSplit(BigInteger.ONE, f.request.recipient.canonicalPayload())
        assertFailsWith<IllegalArgumentException> { core.outboxRelease(f.terminal, redeem, f.paymentBytes,
            KagemushaDeviceSenderTerminalReceiptV1.PaymentAcknowledgement(f.ackBytes), f.q) }
        assertEquals(0, endpoint.calls)
    }

    @Test fun `native admission requires the original canonical authenticator field`() {
        val f = Fixture()
        val endpoint = Endpoint()
        val core = KagemushaNativeCoreCoordinatorAdapterV1.openEndpoint("/test/store", endpoint)
        listOf(byteArrayOf(), ByteArray(63), ByteArray(64), ByteArray(65)).forEach { invalid ->
            assertFailsWith<IllegalArgumentException> { core.acceptAuthenticatedDeviceReply(
                5, f.id, byteArrayOf(7), byteArrayOf(8), invalid, f.q) }
        }
        val retired = listOf(u32(5), f.id, byteArrayOf(7), byteArrayOf(8)) + f.qFields
        assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.encodeRequest(
            KagemushaCoreCoordinatorMethodV1.ACCEPT_AUTHENTICATED_REPLY, retired) }
        assertEquals(0, endpoint.calls)
    }

    @Test fun `provider preserves original authenticators through qualification and normal admission`() {
        val f = Fixture()
        val qualificationReply = testArchive("iroha.kagemusha.device.v1.active-hardware-credential-reply",
            testFields(byteArrayOf(1, 0), byteArrayOf(1), f.q.releaseId(), f.q.hardwarePolicyDigest(),
                f.q.coreAuthorizationKeyReference(), NoritoHeader.decode(f.qFields[2], null).payload,
                NoritoHeader.decode(f.qFields[3], null).payload))
        val requestReply = testArchive("iroha.kagemusha.device.v1.signed-payment-request-reply",
            testFields(byteArrayOf(1, 0), byteArrayOf(22),
                ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(f.requestBytes.size.toLong()).array() + f.requestBytes))
        val admitted = mutableListOf<Pair<Int, ByteArray>>()
        val endpoint = object : KagemushaCoreCoordinatorEndpointV1 {
            override fun contract() = intArrayOf(2, 23, 3, 6, 50, 8, 6, 22, 16, 0xffff)
            override fun open(storagePath: String) = 1L
            override fun invoke(handle: Long, method: Int, fields: Array<ByteArray>): Array<ByteArray> = when (method) {
                1 -> arrayOf(fields[1])
                2 -> emptyArray()
                3 -> { admitted += fields[0][0].toInt() to fields[4].copyOf(); emptyArray() }
                else -> error("unexpected method $method")
            }
        }
        fun authenticator(operation: Int) = ByteArray(64).also { it[31] = 1; it[63] = operation.toByte() }
        val transport = object : KagemushaNativeAuthenticatedDeviceTransportV1 {
            override fun hardwarePolicyId() = f.q.hardwarePolicyDigest()
            override fun qualificationReportDigest() = f.q.profile.qualificationReportDigest()
            override fun executeAndVerify(operation: Int, requestId: ByteArray, canonicalCommand: ByteArray,
                acceptedDevicePublicKey: ByteArray?): KagemushaAuthenticatedDeviceResponseV1 {
                if (operation == 1) assertNull(acceptedDevicePublicKey)
                else assertContentEquals(f.credential.devicePublicKey.sec1Bytes(), acceptedDevicePublicKey)
                return KagemushaAuthenticatedDeviceResponseV1(operation, KagemushaAuthenticatedDeviceStatusV1.SUCCESS,
                    if (operation == 1) qualificationReply else requestReply, authenticator(operation))
            }
        }
        val provider = KagemushaAuthenticatedHardwareProviderV1(transport,
            KagemushaNativeCoreCoordinatorAdapterV1.openEndpoint("/test/store", endpoint))
        assertContentEquals(f.requestBytes, provider.createPaymentRequest(f.request.requestId(),
            f.request.recipient.canonicalPayload(), f.request.amount, f.request.expiresAtMs - f.request.issuedAtMs))
        assertEquals(listOf(1, 22), admitted.map { it.first })
        admitted.forEach { assertContentEquals(authenticator(it.first), it.second) }
    }

    @Test fun `terminal and recovery envelopes reject one byte beyond the native bound`() {
        val f = Fixture()
        assertFailsWith<IllegalArgumentException> {
            responding(6, listOf(ByteArray(7_937))).terminalEnvelope(f.candidate, byteArrayOf(7))
        }
        val recovery = KagemushaCoreCoordinatorArchiveV1.decodeRecoveryShapeExact(f.recoveryBytes)
        assertFailsWith<IllegalArgumentException> {
            responding(9, listOf(ByteArray(7_937))).recoverTerminalEnvelope(recovery, byteArrayOf(10))
        }
    }

    @Test fun `provider releases retained context after policy and Core key rotation`() {
        verifyRotatedRelease(substitute = false)
    }

    @Test fun `provider rejects substituted release inputs before hardware even across rotation`() {
        verifyRotatedRelease(substitute = true)
    }

    private fun verifyRotatedRelease(substitute: Boolean) {
        val f = Fixture()
        val rotated = f.qualification(2)
        val active = KagemushaHardwareQualificationV1(1, rotated.profile, rotated.credential, rotated.releaseId(),
            digest(88), digest(89), rotated.capabilities())
        val qualificationReply = testArchive("iroha.kagemusha.device.v1.active-hardware-credential-reply",
            testFields(byteArrayOf(1, 0), byteArrayOf(1), active.releaseId(), active.hardwarePolicyDigest(),
                active.coreAuthorizationKeyReference(), NoritoHeader.decode(KagemushaNoritoV1.encodeHardwareProfileShape(active.profile), null).payload,
                NoritoHeader.decode(KagemushaNoritoV1.encodeHardwareCredentialShape(active.credential), null).payload))
        val endpoint = object : KagemushaCoreCoordinatorEndpointV1 {
            override fun contract() = intArrayOf(2, 23, 3, 6, 50, 8, 6, 22, 16, 0xffff)
            override fun open(storagePath: String) = 1L
            override fun invoke(handle: Long, method: Int, fields: Array<ByteArray>): Array<ByteArray> = when (method) {
                1 -> arrayOf(fields[1])
                2, 3 -> emptyArray()
                10 -> arrayOf(f.id, f.preparationBytes, f.envelopeDigest, f.paymentBytes, byteArrayOf(12))
                else -> error("unexpected method $method")
            }
        }
        var releaseReachedDevice = false
        val transport = object : KagemushaNativeAuthenticatedDeviceTransportV1 {
            override fun hardwarePolicyId() = active.hardwarePolicyDigest()
            override fun qualificationReportDigest() = active.profile.qualificationReportDigest()
            override fun executeAndVerify(operation: Int, requestId: ByteArray, canonicalCommand: ByteArray,
                acceptedDevicePublicKey: ByteArray?): KagemushaAuthenticatedDeviceResponseV1 {
                if (operation == 1) return KagemushaAuthenticatedDeviceResponseV1(operation,
                    KagemushaAuthenticatedDeviceStatusV1.SUCCESS, qualificationReply,
                    ByteArray(64).also { it[31] = 1; it[63] = 1 })
                assertEquals(12, operation)
                assertContentEquals(active.credential.devicePublicKey.sec1Bytes(), acceptedDevicePublicKey)
                val command = KagemushaDeviceOperationCodecV1.decodeSenderCommand(12, f.id, canonicalCommand)
                assertContentEquals(f.preparationBytes, KagemushaCoreCoordinatorArchiveV1.encodePreparationShape(
                    KagemushaNativeSenderPreparationV1(f.id, command.context, f.preparation.inputsDigest())))
                releaseReachedDevice = true
                // Stop before any hardware monetary success; this is an orchestration test.
                return KagemushaAuthenticatedDeviceResponseV1(operation, KagemushaAuthenticatedDeviceStatusV1.UNAVAILABLE,
                    byteArrayOf(), byteArrayOf())
            }
        }
        val adapter = KagemushaNativeCoreCoordinatorAdapterV1.openEndpoint("/test/store", endpoint)
        val core = if (!substitute) adapter else object : KagemushaNativeCoreCoordinatorV1 by adapter {
            override fun outboxRelease(creditId: ByteArray, inputs: KagemushaDeviceSenderPublicInputsV1, canonicalPayment: ByteArray,
                terminalReceipt: KagemushaDeviceSenderTerminalReceiptV1, qualification: KagemushaHardwareQualificationV1): KagemushaNativeOutboxReleaseV1 =
                KagemushaNativeOutboxReleaseV1(f.id, f.context(), f.preparation.inputsDigest(), f.envelopeDigest,
                    KagemushaDeviceSenderPublicInputsV1.RedeemSplit(BigInteger.ONE, f.request.recipient.canonicalPayload()),
                    f.paymentBytes, byteArrayOf(12))
        }
        val provider = KagemushaAuthenticatedHardwareProviderV1(transport, core)
        assertFailsWith<IllegalArgumentException> {
            provider.recordAcknowledgement(f.terminal, f.requestBytes, f.paymentBytes, f.ackBytes)
        }
        assertEquals(!substitute, releaseReachedDevice)
    }

    private fun responding(method: Int, response: List<ByteArray>): KagemushaNativeCoreCoordinatorAdapterV1 =
        KagemushaNativeCoreCoordinatorAdapterV1.openEndpoint("/test/store", Endpoint().apply { expect(method, null, response) })

    private class Endpoint : KagemushaCoreCoordinatorEndpointV1 {
        var calls = 0
        private var method = 0
        private var request: List<ByteArray>? = null
        private var response = emptyList<ByteArray>()
        fun expect(method: Int, request: List<ByteArray>?, response: List<ByteArray>) {
            this.method = method; this.request = request; this.response = response
        }
        override fun contract() = intArrayOf(2, 23, 3, 6, 50, 8, 6, 22, 16, 0xffff)
        override fun open(storagePath: String) = 1L
        override fun invoke(handle: Long, method: Int, fields: Array<ByteArray>): Array<ByteArray> {
            calls++
            assertEquals(this.method, method)
            request?.let { expected ->
                assertEquals(expected.size, fields.size)
                expected.indices.forEach { assertContentEquals(expected[it], fields[it], "method $method field $it") }
            }
            return response.map { it.copyOf() }.toTypedArray()
        }
    }

    private class Fixture {
        val requestBytes = fixture("payment_request")
        val paymentBytes = fixture("payment")
        val ackBytes = fixture("acknowledgement")
        val request = KagemushaNoritoV1.decodePaymentRequestShapeExact(requestBytes)
        val credential = request.hardwareCredential
        val terminal = KagemushaNoritoV1.decodePaymentShapeExact(paymentBytes, request).output.creditId()
        val id = digest(60)
        val inputs = KagemushaDeviceSenderPublicInputsV1.SendSplit(requestBytes)
        val q = qualification()
        val qFields = listOf(u32(1), q.releaseId(), KagemushaNoritoV1.encodeHardwareProfileShape(q.profile),
            KagemushaNoritoV1.encodeHardwareCredentialShape(q.credential), u32(0xffff))
        val preparation = KagemushaNativeSenderPreparationV1(id, context(),
            KagemushaCoreCoordinatorArchiveV1.inputsDigestShape(id, context(), inputs))
        val preparationBytes = KagemushaCoreCoordinatorArchiveV1.encodePreparationShape(preparation)
        val candidate = KagemushaNativeSenderCandidateV1(preparation,
            KagemushaDeviceSenderPreparationSelectorV1(preparation.inputsDigest(), digest(61)), digest(62), byteArrayOf(7))
        val candidateBytes = KagemushaCoreCoordinatorArchiveV1.encodeCandidateShape(candidate)
        val recoveryBytes = KagemushaCoreCoordinatorArchiveV1.encodeRecoveryShape(
            KagemushaNativeSenderRecoveryV1(id, terminal, context(), preparation.inputsDigest()))
        val envelopeDigest = KagemushaCoreCoordinatorArchiveV1.terminalEnvelopeDigestShape(paymentBytes)
        val aggregateBytes = aggregate()

        fun context(release: ByteArray = request.releaseId(), credential: ByteArray = this.credential.credentialId(),
            coreKey: ByteArray = digest(65), lane: ByteArray = this.credential.laneCommitment(),
            generation: BigInteger = BigInteger.valueOf(this.credential.hardwareEpochGeneration)) =
            KagemushaDeviceSenderWalletContextV1(
                KagemushaDeviceLaneIdV1(request.networkId.bytes(), lane, request.asset.canonicalPayload(), request.scale),
                KagemushaDeviceStateContextV1(1, this.credential.suiteId(), digest(64), release,
                    request.assetIncarnation.bytes(), this.credential.hardwareProfileId(), this.credential.policyEpoch),
                credential, KagemushaDeviceHardwareEpochV1(generation, this.credential.hardwareEpochId()),
                KagemushaDevicePolicyBindingV1(this.credential.deviceKeyReference(), digest(66)), coreKey)

        fun qualification(generation: Long = credential.hardwareEpochGeneration): KagemushaHardwareQualificationV1 {
            val active = KagemushaHardwareCredentialV1(1, credential.credentialId(), credential.networkId,
                credential.hardwareProfileId(), credential.suiteId(), credential.firmwarePolicyDigest(), credential.policyEpoch,
                credential.laneCommitment(), if (generation == credential.hardwareEpochGeneration) credential.hardwareEpochId() else digest(77),
                generation, credential.devicePublicKey, credential.deviceKeyReference(), credential.issuedAtMs,
                credential.expiresAtMs, credential.governanceSignature)
            val profile = KagemushaHardwareProfileV1(1, 1, active.hardwareProfileId(), digest(67),
                KagemushaHardwarePlatformClassV1.ANDROID_OEM_SERVICE, digest(68), active.firmwarePolicyDigest(),
                digest(69), digest(70), digest(71), active.policyEpoch, active.devicePublicKey, 0xffff, digest(72), 1, 20000)
            return KagemushaHardwareQualificationV1(1, profile, active, request.releaseId(), digest(66), digest(65),
                EnumSet.allOf(KagemushaHardwareCapabilityV1::class.java))
        }

        fun aggregate(lane: ByteArray = credential.laneCommitment()): ByteArray = KagemushaNoritoV1.encodeAggregateStateShape(
            KagemushaAggregateStateCommitmentV1(1, request.releaseId(), request.networkId, request.asset, request.assetIncarnation,
                request.scale, request.liabilityPoolId(), lane, credential.hardwareEpochId(), credential.deviceKeyReference(),
                digest(66), BigInteger.ONE, digest(73)))
    }

    companion object {
        private fun digest(value: Int) = ByteArray(32) { value.toByte() }
        private fun u32(value: Int) = KagemushaCoreCoordinatorFrameV1.u32(value)
        private fun testFields(vararg values: ByteArray): ByteArray = values.fold(byteArrayOf()) { result, value ->
            var size = value.size
            val length = ArrayList<Byte>()
            do {
                val current = size and 127
                size = size ushr 7
                length += (current or if (size != 0) 128 else 0).toByte()
            } while (size != 0)
            result + length.toByteArray() + value
        }
        private fun testArchive(schema: String, payload: ByteArray): ByteArray = NoritoCodec.encode(payload, schema,
            object : TypeAdapter<ByteArray> {
                override fun encode(encoder: NoritoEncoder, value: ByteArray) = encoder.writeBytes(value)
                override fun decode(decoder: NoritoDecoder) = decoder.readBytes(decoder.remaining())
            }) // These control schemas align to eight; the 40-byte header needs no padding.
        private fun fixture(section: String): ByteArray {
            var directory = Paths.get("").toAbsolutePath().normalize()
            while (directory != null) {
                val file = directory.resolve("fixtures/offline/kagemusha_v1.json")
                if (Files.isRegularFile(file)) {
                    val json = String(Files.readAllBytes(file), Charsets.UTF_8)
                    val hex = requireNotNull(Regex("\"$section\"\\s*:\\s*\\{.*?\"norito_hex\"\\s*:\\s*\"([^\"]+)\"",
                        RegexOption.DOT_MATCHES_ALL).find(json)).groupValues[1]
                    return hex.chunked(2).map { it.toInt(16).toByte() }.toByteArray()
                }
                directory = directory.parent
            }
            error("missing KAGEMUSHA fixture")
        }
    }
}
