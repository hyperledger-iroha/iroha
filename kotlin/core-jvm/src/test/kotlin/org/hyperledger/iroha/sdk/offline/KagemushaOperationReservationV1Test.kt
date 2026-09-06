// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Paths
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class KagemushaOperationReservationV1Test {
    @Test
    fun `sender reservations match the shared canonical native Core bindings`() {
        val fixture = fixture()
        val core = RecordingCore()
        val transport = UnavailableTransport()
        val provider = KagemushaAuthenticatedHardwareProviderV1(transport, core)
        val id = ByteArray(32) { 7 }
        val request = bytes(fixture, "send_request_hex")
        assertContentEquals(id, provider.reservePaymentOperationId(id, request))
        assertContentEquals(bytes(fixture, "send_binding_hex"), core.bindings.last())
        assertContentEquals(
            id,
            provider.reserveRedemptionOperationId(
                id, BigInteger(value(fixture, "redeem_amount_decimal")),
                bytes(fixture, "redeem_beneficiary_payload_hex"),
            ),
        )
        assertContentEquals(bytes(fixture, "redeem_binding_hex"), core.bindings.last())
        assertEquals(listOf(5, 5), core.operations)
        assertEquals(0, transport.calls)
    }

    @Test
    fun `request and mint reservations retain caller identity and exact intent`() {
        val request = KagemushaNoritoV1.decodePaymentRequestShapeExact(bytes(fixture(), "send_request_hex"))
        val id = ByteArray(32) { 8 }
        val core = RecordingCore()
        val provider = KagemushaAuthenticatedHardwareProviderV1(UnavailableTransport(), core)
        repeat(2) {
            assertContentEquals(id, provider.reservePaymentRequestOperationId(id, request.recipient.canonicalPayload(), request.amount, 1000L))
        }
        assertContentEquals(core.bindings[0], core.bindings[1])
        assertContentEquals(
            KagemushaDeviceOperationCodecV1.encodeControlCommand(
                KagemushaDeviceControlCommandV1.CreateSignedPaymentRequest(id, request.recipient, request.amount, 1000L),
            ), core.bindings[0],
        )
        assertContentEquals(id, provider.reserveMintOperationId(id, request.amount, request.recipient.canonicalPayload(), request.recipient.canonicalPayload()))
        assertEquals(listOf(22, 22, 14), core.operations)
        core.ids.forEach { assertContentEquals(id, it) }
    }

    @Test
    fun `substituted or malformed identities fail before device execution`() {
        val request = KagemushaNoritoV1.decodePaymentRequestShapeExact(bytes(fixture(), "send_request_hex"))
        val core = RecordingCore().also { it.substituteId = true }
        val transport = UnavailableTransport()
        val provider = KagemushaAuthenticatedHardwareProviderV1(transport, core)
        val id = ByteArray(32) { 9 }
        assertFailsWith<IllegalArgumentException> { provider.qualification() }
        assertFailsWith<IllegalArgumentException> { provider.reservePaymentOperationId(id, bytes(fixture(), "send_request_hex")) }
        assertFailsWith<IllegalArgumentException> { provider.reservePaymentRequestOperationId(id, request.recipient.canonicalPayload(), request.amount, 1000L) }
        assertFailsWith<IllegalArgumentException> { provider.reserveMintOperationId(id, request.amount, request.recipient.canonicalPayload(), request.recipient.canonicalPayload()) }
        assertFailsWith<IllegalArgumentException> { provider.reserveRedemptionOperationId(id, request.amount, request.recipient.canonicalPayload()) }
        assertFailsWith<IllegalArgumentException> { provider.reservePaymentOperationId(ByteArray(32), bytes(fixture(), "send_request_hex")) }
        assertEquals(0, transport.calls)
    }

    @Test
    fun `wallet boundary rejects a provider that substitutes or mutates the caller identity`() {
        val id = ByteArray(32) { 10 }
        assertContentEquals(id, reserveKagemushaOperationIdV1(id) { it })
        assertFailsWith<IllegalArgumentException> {
            reserveKagemushaOperationIdV1(id) { ByteArray(32) { 11 } }
        }
        assertFailsWith<IllegalArgumentException> {
            reserveKagemushaOperationIdV1(id) { it.apply { fill(11) } }
        }
        assertContentEquals(ByteArray(32) { 10 }, id)
        var called = false
        assertFailsWith<IllegalArgumentException> {
            reserveKagemushaOperationIdV1(ByteArray(32)) { called = true; it }
        }
        assertEquals(false, called)
    }

    @Test
    fun `request execution reserves exact caller intent before reaching hardware`() {
        val request = KagemushaNoritoV1.decodePaymentRequestShapeExact(bytes(fixture(), "send_request_hex"))
        val core = RecordingCore().also { it.substituteId = true }
        val transport = UnavailableTransport()
        val provider = KagemushaAuthenticatedHardwareProviderV1(transport, core)
        val id = ByteArray(32) { 12 }
        assertFailsWith<IllegalArgumentException> {
            provider.createPaymentRequest(id, request.recipient.canonicalPayload(), request.amount, 1000L)
        }
        assertEquals(listOf(22), core.operations)
        assertContentEquals(id, core.ids.single())
        assertEquals(0, transport.calls)
    }

    private fun fixture(): String {
        var directory = Paths.get("").toAbsolutePath().normalize()
        while (directory != null) {
            val path = directory.resolve("fixtures/offline/kagemusha_sender_reservation_v1.json")
            if (Files.isRegularFile(path)) return String(Files.readAllBytes(path), StandardCharsets.UTF_8)
            directory = directory.parent
        }
        error("missing sender reservation fixture")
    }

    private fun value(fixture: String, key: String): String =
        Regex("\\\"${Regex.escape(key)}\\\"\\s*:\\s*\\\"([^\\\"]+)\\\"").find(fixture)!!.groupValues[1]

    private fun bytes(fixture: String, key: String): ByteArray =
        value(fixture, key).chunked(2).map { it.toInt(16).toByte() }.toByteArray()

    private class UnavailableTransport : KagemushaNativeAuthenticatedDeviceTransportV1 {
        var calls = 0
        override fun hardwarePolicyId() = ByteArray(32) { 2 }
        override fun qualificationReportDigest() = ByteArray(32) { 3 }
        override fun executeAndVerify(operation: Int, requestId: ByteArray, canonicalCommand: ByteArray, acceptedDevicePublicKey: ByteArray?): KagemushaAuthenticatedDeviceResponseV1 {
            calls++
            return KagemushaAuthenticatedDeviceResponseV1(operation, KagemushaAuthenticatedDeviceStatusV1.UNAVAILABLE, ByteArray(0), ByteArray(0))
        }
    }

    private class RecordingCore : KagemushaNativeCoreCoordinatorV1 {
        var substituteId = false
        val operations = mutableListOf<Int>()
        val ids = mutableListOf<ByteArray>()
        val bindings = mutableListOf<ByteArray>()
        override fun reserveOperationId(operation: Int, operationId: ByteArray, publicBinding: ByteArray): ByteArray {
            operations.add(operation)
            ids.add(operationId.copyOf())
            bindings.add(publicBinding.copyOf())
            return if (substituteId) ByteArray(32) { -1 } else operationId.copyOf()
        }
        override fun acceptQualification(qualification: KagemushaHardwareQualificationV1, hardwarePolicyDigest: ByteArray): Unit = error("unused")
        override fun acceptAuthenticatedDeviceReply(operation: Int, requestId: ByteArray, canonicalCommand: ByteArray, canonicalReply: ByteArray, qualification: KagemushaHardwareQualificationV1): Unit = error("unused")
        override fun beginSenderTransition(operationId: ByteArray, inputs: KagemushaDeviceSenderPublicInputsV1, qualification: KagemushaHardwareQualificationV1): KagemushaNativeSenderPreparationV1 = error("unused")
        override fun provePreparedSenderTransition(preparation: KagemushaNativeSenderPreparationV1, authenticatedPreparationReply: ByteArray): KagemushaNativeSenderCandidateV1 = error("unused")
        override fun terminalEnvelope(candidate: KagemushaNativeSenderCandidateV1, authenticatedCommitReply: ByteArray): ByteArray = error("unused")
        override fun acceptInstalledTerminal(candidate: KagemushaNativeSenderCandidateV1, canonicalEnvelope: ByteArray, authenticatedInstallReply: ByteArray, authenticatedInstalledReply: ByteArray, authenticatedWalletSnapshotReply: ByteArray): KagemushaHardwareTerminalResultV1 = error("unused")
        override fun senderRecovery(kind: KagemushaNativeSenderKindV1, terminalId: ByteArray, qualification: KagemushaHardwareQualificationV1): KagemushaNativeSenderRecoveryV1? = error("unused")
        override fun senderRecoveryByOperationId(kind: KagemushaNativeSenderKindV1, operationId: ByteArray, qualification: KagemushaHardwareQualificationV1): KagemushaNativeSenderRecoveryV1? = error("unused")
        override fun recoverTerminalEnvelope(recovery: KagemushaNativeSenderRecoveryV1, authenticatedInstalledReply: ByteArray): ByteArray = error("unused")
        override fun outboxRelease(creditId: ByteArray, inputs: KagemushaDeviceSenderPublicInputsV1, canonicalPayment: ByteArray, terminalReceipt: KagemushaDeviceSenderTerminalReceiptV1, qualification: KagemushaHardwareQualificationV1): KagemushaNativeOutboxReleaseV1 = error("unused")
    }
}
