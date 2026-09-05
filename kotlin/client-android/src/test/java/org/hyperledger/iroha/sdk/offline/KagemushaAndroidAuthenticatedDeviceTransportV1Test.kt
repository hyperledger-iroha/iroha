// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

class KagemushaAndroidAuthenticatedDeviceTransportV1Test {
    @Test
    fun `online-only bridge cannot become an authenticated transport`() {
        assertFailsWith<IllegalStateException> {
            KagemushaAndroidAuthenticatedDeviceTransportV1(
                KagemushaDeviceLifecycleBridgeV1.onlineOnly(),
            )
        }
    }

    @Test
    fun `all 22 lifecycle operations retain authenticated bindings`() {
        val endpoint = FakeEndpoint()
        val bridge = KagemushaDeviceLifecycleBridgeV1.withEndpointForTests(endpoint)
        val transport = KagemushaAndroidAuthenticatedDeviceTransportV1(bridge)

        assertContentEquals(fixed(0x22, 32), transport.hardwarePolicyId())
        assertContentEquals(fixed(0x33, 32), transport.qualificationReportDigest())

        for (operation in 1..22) {
            endpoint.operation = KagemushaDeviceLifecycleBridgeV1.Operation.values()
                .single { candidate -> candidate.code == operation }
            endpoint.status = KagemushaDeviceLifecycleBridgeV1.Status.SUCCESS
            val acceptedKey = if (operation == 1) null else devicePublicKey()
            val response = transport.executeAndVerify(
                operation,
                fixed(operation, 32),
                byteArrayOf(operation.toByte()),
                acceptedKey,
            )
            assertEquals(operation, response.operation)
            assertEquals(KagemushaAuthenticatedDeviceStatusV1.SUCCESS, response.status)
            assertContentEquals(byteArrayOf(operation.toByte(), 0x5a), response.canonicalReply())
            assertContentEquals(validLowSSignature(), response.authenticator())
            assertEquals(endpoint.operation, endpoint.lastVerifiedOperation)
            assertContentEquals(fixed(operation, 32), endpoint.lastVerifiedRequestId!!)
            assertContentEquals(fixed(0x22, 32), endpoint.lastVerifiedHardwarePolicyId!!)
            assertContentEquals(
                fixed(0x33, 32),
                endpoint.lastVerifiedQualificationReportDigest!!,
            )
            if (acceptedKey == null) {
                assertEquals(null, endpoint.lastVerifiedDevicePublicKey)
            } else {
                assertContentEquals(acceptedKey, endpoint.lastVerifiedDevicePublicKey!!)
            }
            assertTrue(endpoint.lastResponse!!.all { byte -> byte == 0.toByte() })
        }
        assertEquals((1..22).toList(), endpoint.observedOperations)
        assertEquals(22, endpoint.verifierCalls)
    }

    @Test
    fun `non-success statuses expose no response bytes`() {
        val endpoint = FakeEndpoint().apply {
            operation = KagemushaDeviceLifecycleBridgeV1.Operation.RECOVER_TERMINAL_OUTCOME
            status = KagemushaDeviceLifecycleBridgeV1.Status.RECOVERY_REQUIRED
        }
        val transport = KagemushaAndroidAuthenticatedDeviceTransportV1(
            KagemushaDeviceLifecycleBridgeV1.withEndpointForTests(endpoint),
        )
        val response = transport.executeAndVerify(
            operation = 8,
            requestId = fixed(0x08, 32),
            canonicalCommand = byteArrayOf(0x08),
            acceptedDevicePublicKey = devicePublicKey(),
        )
        assertEquals(KagemushaAuthenticatedDeviceStatusV1.RECOVERY_REQUIRED, response.status)
        assertContentEquals(byteArrayOf(), response.canonicalReply())
        assertContentEquals(byteArrayOf(), response.authenticator())
        assertEquals(0, endpoint.verifierCalls)
    }

    @Test
    fun `rejected native authenticator never exposes a successful response`() {
        val endpoint = FakeEndpoint().apply {
            operation = KagemushaDeviceLifecycleBridgeV1.Operation.STAGE_INBOUND_PAYMENT
            verificationResult = false
        }
        val transport = KagemushaAndroidAuthenticatedDeviceTransportV1(
            KagemushaDeviceLifecycleBridgeV1.withEndpointForTests(endpoint),
        )

        assertFailsWith<IllegalArgumentException> {
            transport.executeAndVerify(
                operation = 2,
                requestId = fixed(0x12, 32),
                canonicalCommand = byteArrayOf(0x12),
                acceptedDevicePublicKey = devicePublicKey(),
            )
        }

        assertEquals(1, endpoint.verifierCalls)
        assertTrue(endpoint.lastResponse!!.all { byte -> byte == 0.toByte() })
    }

    @Test
    fun `unknown operations fail before the secure element is called`() {
        val endpoint = FakeEndpoint()
        val transport = KagemushaAndroidAuthenticatedDeviceTransportV1(
            KagemushaDeviceLifecycleBridgeV1.withEndpointForTests(endpoint),
        )
        for (operation in listOf(Int.MIN_VALUE, 0, 23, Int.MAX_VALUE)) {
            assertFailsWith<IllegalArgumentException> {
                transport.executeAndVerify(
                    operation,
                    fixed(0x11, 32),
                    byteArrayOf(1),
                    devicePublicKey(),
                )
            }
        }
        assertTrue(endpoint.observedOperations.isEmpty())
    }

    private class FakeEndpoint : KagemushaDeviceLifecycleBridgeV1.Endpoint {
        var operation = KagemushaDeviceLifecycleBridgeV1.Operation.READ_ACTIVE_HARDWARE_CREDENTIAL
        var status = KagemushaDeviceLifecycleBridgeV1.Status.SUCCESS
        var verificationResult = true
        var lastResponse: ByteArray? = null
        var verifierCalls = 0
        var lastVerifiedOperation: KagemushaDeviceLifecycleBridgeV1.Operation? = null
        var lastVerifiedRequestId: ByteArray? = null
        var lastVerifiedHardwarePolicyId: ByteArray? = null
        var lastVerifiedQualificationReportDigest: ByteArray? = null
        var lastVerifiedDevicePublicKey: ByteArray? = null
        val observedOperations = mutableListOf<Int>()

        override fun capabilities(): ByteArray =
            KagemushaDeviceLifecycleBridgeV1.Codec.encodeCapabilitiesForTests(
                platform = 1,
                policy = fixed(0x22, 32),
                attestation = fixed(0x33, 32),
            )

        override fun execute(command: ByteArray): ByteArray {
            val observedOperation = command[10].toInt() and 0xff
            observedOperations += observedOperation
            val requestId = command.copyOfRange(12, 44)
            val success = status == KagemushaDeviceLifecycleBridgeV1.Status.SUCCESS
            return KagemushaDeviceLifecycleBridgeV1.Codec.encodeResponseForTests(
                operation = operation,
                status = status,
                requestId = requestId,
                payload = if (success) byteArrayOf(observedOperation.toByte(), 0x5a) else byteArrayOf(),
                authenticator = if (success) validLowSSignature() else byteArrayOf(),
            ).also { response -> lastResponse = response }
        }

        override fun verifyResponseAuthenticator(
            response: ByteArray,
            operation: KagemushaDeviceLifecycleBridgeV1.Operation,
            requestId: ByteArray,
            hardwarePolicyId: ByteArray,
            qualificationReportDigest: ByteArray,
            acceptedDevicePublicKey: ByteArray?,
        ): Boolean {
            verifierCalls += 1
            lastVerifiedOperation = operation
            lastVerifiedRequestId = requestId.copyOf()
            lastVerifiedHardwarePolicyId = hardwarePolicyId.copyOf()
            lastVerifiedQualificationReportDigest = qualificationReportDigest.copyOf()
            lastVerifiedDevicePublicKey = acceptedDevicePublicKey?.copyOf()
            return verificationResult
        }
    }

    companion object {
        private fun fixed(value: Int, count: Int): ByteArray = ByteArray(count) { value.toByte() }

        private fun devicePublicKey(): ByteArray = byteArrayOf(4) + fixed(0x55, 64)

        private fun validLowSSignature(): ByteArray = fixed(0x11, 32) + fixed(0x22, 32)
    }
}
