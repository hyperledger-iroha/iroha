// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.KeyPairGenerator
import java.security.MessageDigest
import java.security.interfaces.ECPublicKey
import java.security.spec.ECGenParameterSpec
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidAppKeyHardwarePolicyV1
import org.junit.jupiter.api.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull

/** Scripted ABI fixtures test JVM correlation/once-only control flow, not installed native authority. */
class KagemushaNativeAppApprovalCoordinatorV1Test {
    @Test fun `native fence precedes exactly one signing call and retries return retained receipt`() {
        val endpoint = Endpoint(); val prepared = facade(endpoint).prepareApproval(endpoint.id)
        var signs = 0
        val receipt = prepared.performPlatformSigning { alias, challenge, point, key, message, policy, guard ->
            signs++; assertEquals(1, endpoint.state); assertEquals(endpoint.fields[3].toString(Charsets.UTF_8), alias)
            assertContentEquals(endpoint.fields[4], challenge); assertContentEquals(endpoint.fields[5], point)
            assertContentEquals(endpoint.fields[6], key); assertContentEquals(endpoint.fields[1], message)
            assertEquals(KagemushaAndroidAppKeyHardwarePolicyV1.TEE_ONLY, policy)
            guard(); der.copyOf()
        }
        assertEquals(1, signs); assertContentEquals(der, endpoint.raw)
        assertContentEquals(receipt, prepared.performPlatformSigning { _, _, _, _, _, _, _ -> error("Must not sign again") })
        assertContentEquals(receipt, prepared.recoverOriginalApproval())
        receipt.fill(0)
        assertEquals(1, signs); assertContentEquals(endpoint.receipt(), prepared.recoverOriginalApproval())
    }

    @Test fun `retained original recovery consumes without invoking the platform`() {
        val endpoint = Endpoint().apply { state = 2; raw = der.copyOf() }
        val prepared = facade(endpoint).prepareApproval(endpoint.id)
        assertContentEquals(endpoint.receipt(), prepared.performPlatformSigning { _, _, _, _, _, _, _ -> error("Must recover") })
        assertEquals(3, endpoint.state); assertEquals(0, endpoint.retains)
    }

    @Test fun `uncertain platform outcome freezes rather than asking for another signature`() {
        val endpoint = Endpoint(); val prepared = facade(endpoint).prepareApproval(endpoint.id)
        var calls = 0
        assertFailsWith<IllegalStateException> {
            prepared.performPlatformSigning { _, _, _, _, _, _, _ -> calls++; error("Platform result lost") }
        }
        assertEquals(1, endpoint.state)
        assertFailsWith<IllegalStateException> { prepared.performPlatformSigning { _, _, _, _, _, _, _ -> calls++; der.copyOf() } }
        assertFailsWith<IllegalStateException> { prepared.recoverOriginalApproval() }
        assertEquals(1, calls)
    }

    @Test fun `retention lost return closes old handle and a fresh fixture owner recovers only original`() {
        val endpoint = Endpoint().apply { loseRetainReturn = true }
        val prepared = facade(endpoint).prepareApproval(endpoint.id)
        assertFailsWith<IllegalStateException> { prepared.performPlatformSigning { _, _, _, _, _, _, _ -> der.copyOf() } }
        assertEquals(2, endpoint.state); assertEquals(1, endpoint.closes); assertContentEquals(der, endpoint.raw)
        endpoint.loseRetainReturn = false
        val recovered = facade(endpoint).prepareApproval(endpoint.id)
        assertContentEquals(endpoint.receipt(), recovered.performPlatformSigning { _, _, _, _, _, _, _ -> error("No fresh signature") })
    }

    @Test fun `native scope substitution is refused before invocation fence`() {
        val endpoint = Endpoint(); val prepared = facade(endpoint).prepareApproval(endpoint.id)
        endpoint.substituteScope = true
        assertFailsWith<IllegalStateException> { prepared.signingBytes() }
        assertEquals(0, endpoint.state); assertEquals(1, endpoint.closes)
    }

    @Test fun `prepared frame rejects key challenge credential subject and legacy C substitutions`() {
        for (index in listOf(4, 6, 8, 13)) {
            val endpoint = Endpoint(); endpoint.fields[index][endpoint.fields[index].lastIndex] =
                (endpoint.fields[index].last().toInt() xor 1).toByte()
            assertFailsWith<IllegalArgumentException> { facade(endpoint).prepareApproval(endpoint.id) }
            assertEquals(1, endpoint.closes); assertEquals(0, endpoint.state)
        }
        for (offset in listOf(155, 323)) {
            val corrupted = Endpoint()
            corrupted.fields[13][offset] = (corrupted.fields[13][offset].toInt() xor 1).toByte()
            // Keep W's SHA(S) correct so only the corrupted enrolled credential/epoch predicate
            // rejects this structurally complete projection before any platform invocation.
            val wStart = "iroha:kagemusha:v1:app-operation-approval\u0000".toByteArray(Charsets.US_ASCII).size + 8
            sha(corrupted.fields[13]).copyInto(corrupted.fields[1], wStart + 3 + 6 * 32)
            assertFailsWith<IllegalArgumentException> { facade(corrupted).prepareApproval(corrupted.id) }
            assertEquals(0, corrupted.state)
        }
        val legacy = Endpoint(); legacy.fields[7] = legacy.fields[7].copyOf(legacy.fields[7].size - 8)
        assertFailsWith<IllegalArgumentException> { facade(legacy).prepareApproval(legacy.id) }
    }

    @Test fun `receipt original substitution cannot be exposed as a completed approval`() {
        val endpoint = Endpoint().apply { state = 2; raw = der.copyOf(); substituteReceipt = true }
        val prepared = facade(endpoint).prepareApproval(endpoint.id)
        assertFailsWith<IllegalStateException> { prepared.recoverOriginalApproval() }
        assertEquals(1, endpoint.closes)
    }

    @Test fun `E uses distinct purpose pending scope and no financial subject or completed credential`() {
        val endpoint = Endpoint(enrollment = true)
        val prepared = facade(endpoint).prepareEnrollmentPossession(endpoint.id)
        assertNull(prepared.recoverOriginalPossession())
        assertContentEquals(endpoint.fields[1], prepared.signingBytes())
        val receipt = prepared.performPlatformSigning { _, _, _, _, message, _, _ ->
            assertContentEquals(endpoint.fields[1], message); der.copyOf()
        }
        assertEquals(2, receipt[10].toInt()); assertContentEquals(endpoint.fields[9], receipt.copyOfRange(147, 179))
        val wrong = Endpoint(enrollment = true); wrong.fields[8] = bytes(7)
        assertFailsWith<IllegalArgumentException> { facade(wrong).prepareEnrollmentPossession(wrong.id) }
        val renewed = Endpoint(enrollment = true)
        val times = "iroha:kagemusha:v1:app-enrollment-possession\u0000".toByteArray(Charsets.US_ASCII).size + 8 + 355
        ByteBuffer.wrap(renewed.fields[1]).order(ByteOrder.LITTLE_ENDIAN).putLong(times, 1001)
        assertFailsWith<IllegalArgumentException> { facade(renewed).prepareEnrollmentPossession(renewed.id) }
        val apple = Endpoint(enrollment = true)
        val cBody = "iroha:kagemusha:v1:ordinary-app-enrollment-challenge\u0000".toByteArray(Charsets.US_ASCII).size + 8
        apple.fields[7][cBody + 2] = 2
        apple.fields[2] = byteArrayOf(4); apple.fields[4] = sha(apple.fields[7])
        val eBody = "iroha:kagemusha:v1:app-enrollment-possession\u0000".toByteArray(Charsets.US_ASCII).size + 8
        apple.id.copyInto(apple.fields[1], eBody + 3)
        apple.fields[3] = java.util.Base64.getEncoder().encodeToString(apple.fields[6]).toByteArray(Charsets.US_ASCII)
        apple.fields[10] = KagemushaCoreCoordinatorFrameV1.u32(0); apple.fields[11] = byteArrayOf(0)
        val applePrepared = facade(apple).prepareEnrollmentPossession(apple.id)
        assertContentEquals(apple.fields[1], applePrepared.signingBytes())
        assertFailsWith<IllegalStateException> { applePrepared.performPlatformSigning { _, _, _, _, _, _, _ -> error("No Android invocation") } }
        assertEquals(0, apple.state)
        val subject = Endpoint(enrollment = true); subject.fields[13] = byteArrayOf(1)
        assertFailsWith<IllegalArgumentException> { facade(subject).prepareEnrollmentPossession(subject.id) }
    }

    @Test fun `E rejects stable enrollment ID and a second challenge hash before signing`() {
        val eBody = "iroha:kagemusha:v1:app-enrollment-possession\u0000".toByteArray(Charsets.US_ASCII).size + 8
        val cBody = "iroha:kagemusha:v1:ordinary-app-enrollment-challenge\u0000".toByteArray(Charsets.US_ASCII).size + 8
        for (stableId in listOf(true, false)) {
            val endpoint = Endpoint(enrollment = true)
            val substituted = if (stableId) endpoint.fields[7].copyOfRange(cBody + 3, cBody + 35)
                else sha(sha(endpoint.fields[7]))
            // Correlate the offered operation and E to each other so only the sole full-C
            // binding rejects this alternate. No native/platform invocation is represented.
            endpoint.offeredId = substituted.copyOf()
            substituted.copyInto(endpoint.fields[1], eBody + 3)
            assertFailsWith<IllegalArgumentException> { facade(endpoint).prepareEnrollmentPossession(substituted) }
            assertEquals(0, endpoint.state); assertEquals(1, endpoint.closes)
        }
    }

    private fun facade(endpoint: Endpoint) = KagemushaNativeAppApprovalCoordinatorV1(
        KagemushaCoreCoordinatorBridgeV1.openEndpoint("/fixture/native-owner", endpoint))

    private class Endpoint(val enrollment: Boolean = false) : KagemushaCoreCoordinatorEndpointV1 {
        val fields = projection(enrollment).toMutableList()
        var offeredId: ByteArray? = null
        val id: ByteArray get() = offeredId?.copyOf() ?: if (enrollment) sha(fields[7]) else bytes(0x11)
        var state = 0 // 0 uninvoked, 1 invoked/no original, 2 retained, 3 consumed
        var raw = byteArrayOf(); var retains = 0; var closes = 0
        var loseRetainReturn = false; var substituteScope = false; var substituteReceipt = false
        override fun contract() = intArrayOf(2, 25, 3, 6, 50, 8, 6, 22, 16, 0xffff, 1, 21)
        override fun install(storagePath: String) = 0
        override fun open(storagePath: String) = 1L
        override fun close(handle: Long): Int { closes++; return 0 }
        override fun invoke(handle: Long, method: Int, request: Array<ByteArray>): Array<ByteArray>? {
            assertEquals(if (enrollment) 20 else 19, method)
            val phase = ByteBuffer.wrap(request[0]).order(ByteOrder.LITTLE_ENDIAN).int
            if (phase != 1) assertContentEquals(fields[0], request[1])
            return when (phase) {
                1 -> { assertContentEquals(id, request[1]); fields.map(ByteArray::copyOf).toTypedArray() }
                2 -> when (state) {
                    0 -> { state = 1; arrayOf(byteArrayOf(1), byteArrayOf(), byteArrayOf()) }
                    2 -> arrayOf(byteArrayOf(2), raw.copyOf(), byteArrayOf())
                    3 -> arrayOf(byteArrayOf(3), raw.copyOf(), receipt())
                    else -> null
                }
                3 -> { check(state == 1); retains++; raw = request[2].copyOf(); state = 2
                    if (loseRetainReturn) null else arrayOf(sha(raw)) }
                4 -> { check(state == 2 || state == 3); state = 3; arrayOf(receipt()) }
                5 -> when (state) {
                    0 -> arrayOf(byteArrayOf(0), byteArrayOf(), byteArrayOf())
                    2 -> arrayOf(byteArrayOf(1), raw.copyOf(), byteArrayOf())
                    3 -> arrayOf(byteArrayOf(2), raw.copyOf(), receipt())
                    else -> null
                }
                6 -> arrayOf(if (substituteScope) bytes(0x42) else fields[9].copyOf(), sha(fields[1]))
                7 -> emptyArray()
                else -> error("Unexpected fixture phase")
            }
        }
        fun receipt(): ByteArray = output {
            write("KGMAPP1\u0000".toByteArray(Charsets.US_ASCII)); write(byteArrayOf(1, 0, if (enrollment) 2 else 1))
            write(fields[0]); write(if (substituteReceipt) bytes(0x77) else id); write(fields[9]); write(sha(fields[1]))
            write(sha(raw)); write(if (enrollment) fields[9] else fields[8]); write(ByteArray(5))
        }
    }
    companion object {
        private val der = byteArrayOf(0x30, 6, 2, 1, 1, 2, 1, 1)
        private fun bytes(marker: Int) = ByteArray(32) { marker.toByte() }
        private fun sha(value: ByteArray) = MessageDigest.getInstance("SHA-256").digest(value)
        private fun le64(value: Long) = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(value).array()
        private fun output(block: ByteArrayOutputStream.() -> Unit) = ByteArrayOutputStream().apply(block).toByteArray()
        private fun framed(domain: String, body: ByteArray) = output {
            write((domain + '\u0000').toByteArray(Charsets.US_ASCII)); write(le64(body.size.toLong())); write(body)
        }
        private fun projection(enrollment: Boolean): List<ByteArray> {
            val pair = KeyPairGenerator.getInstance("EC").apply { initialize(ECGenParameterSpec("secp256r1")) }.generateKeyPair()
            val public = pair.public as ECPublicKey
            fun coordinate(value: BigInteger) = value.toByteArray().takeLast(32).toByteArray().let { ByteArray(32 - it.size) + it }
            val point = byteArrayOf(4) + coordinate(public.w.affineX) + coordinate(public.w.affineY); val key = sha(point)
            val cFields = (1..13).map(::bytes)
            val c = framed("iroha:kagemusha:v1:ordinary-app-enrollment-challenge", output {
                write(byteArrayOf(1, 0, 1)); cFields.forEach { write(it) }
                write(le64(1)); write(le64(1)); write(le64(1000)); write(le64(121000))
            })
            val s = framed("iroha:kagemusha:v1:hardware-transition-selection", output {
                write(byteArrayOf(1, 0)); write(cFields[6]); write(bytes(0x61)); write(bytes(0x62)); write(bytes(0x66))
                write(cFields[4]); write(cFields[5]); write(cFields[7]); write(le64(1)); write(bytes(0x64)); write(le64(1))
                write(byteArrayOf(1)); write(bytes(0x65)); write(ByteArray(64)); write(le64(7)); write(le64(0)); write(le64(8)); write(le64(0))
            })
            val credential = bytes(0x66)
            val subjectFields = if (enrollment) listOf(sha(c), cFields[1], cFields[2], cFields[3], cFields[4],
                cFields[10], cFields[6], cFields[7], cFields[5], key, bytes(0x77)) else listOf(bytes(0x11), bytes(0x22),
                cFields[3], cFields[10], key, credential, sha(s), bytes(0x88))
            val message = framed(if (enrollment) "iroha:kagemusha:v1:app-enrollment-possession" else "iroha:kagemusha:v1:app-operation-approval", output {
                write(byteArrayOf(1, 0, 1)); subjectFields.forEach { write(it) }; write(le64(1000)); write(le64(121000))
            })
            return listOf(le64(7), message, byteArrayOf(5), KagemushaOrdinaryAppKeyAliasV1.originalAlias(c).toByteArray(Charsets.UTF_8), sha(c), point, key, c,
                if (enrollment) byteArrayOf() else credential, bytes(0x99), byteArrayOf(), byteArrayOf(1), bytes(0xaa),
                if (enrollment) byteArrayOf() else s)
        }
    }
}
