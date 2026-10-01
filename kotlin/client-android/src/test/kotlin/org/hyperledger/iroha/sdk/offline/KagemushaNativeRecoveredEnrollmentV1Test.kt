package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Paths
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertSame
import org.junit.jupiter.api.Tag
import org.junit.jupiter.api.Test

/** Scripted JNI validates framing and scope; these fixtures grant no native or hardware authority. */
@Tag("host-native")
class KagemushaNativeRecoveredEnrollmentV1Test {
    @Test fun `recovery retains exact native challenge and proof without initial enrollment`() {
        val endpoint = Endpoint(); val native = open(endpoint); val recovery = native.recoveredEnrollment()
        assertSame(recovery, native.recoveredEnrollment())
        val challenge = recovery.begin()
        assertSame(challenge, recovery.begin())
        challenge.attemptId().fill(0); challenge.accountSigningMessage().fill(0)
        assertContentEquals(u64(7), challenge.attemptId())
        assertContentEquals(KagemushaDeviceOperationCodecV1.encodeControlCommand(
            KagemushaDeviceControlCommandV1.ReadActiveHardwareCredential), challenge.canonicalDeviceCommand())
        val signature = ByteArray(64) { 11 }; val frame = byteArrayOf(12)
        recovery.authenticate(challenge, signature, frame)
        signature.fill(0); frame.fill(0)
        recovery.authenticate(challenge, ByteArray(64) { 11 }, byteArrayOf(12))
        assertFailsWith<IllegalArgumentException> { recovery.authenticate(challenge, ByteArray(64) { 11 }, byteArrayOf(13)) }
        assertEquals(listOf(9, 9, 10, 10), endpoint.phases)
        recovery.cancel(challenge)
        assertEquals(1, endpoint.closed)
        assertEquals(listOf(9, 9, 10, 10), endpoint.phases)
        assertFailsWith<IllegalStateException> { recovery.authenticate(challenge, ByteArray(64) { 11 }, byteArrayOf(12)) }
    }

    @Test fun `cancel retires only the exact outstanding recovery attempt`() {
        val endpoint = Endpoint(); val recovery = open(endpoint).recoveredEnrollment(); val challenge = recovery.begin()
        recovery.cancel(challenge)
        assertEquals(listOf(9, 11), endpoint.phases)
        assertFailsWith<IllegalStateException> { recovery.begin() }
        assertFailsWith<IllegalStateException> { recovery.authenticate(challenge, ByteArray(64), byteArrayOf(1)) }
    }

    @Test fun `foreign challenge and malformed signature cannot dispatch a proof`() {
        val endpoint = Endpoint(); val recovery = open(endpoint).recoveredEnrollment(); val challenge = recovery.begin()
        val foreign = open(Endpoint()).recoveredEnrollment().begin()
        assertFailsWith<IllegalStateException> { recovery.authenticate(foreign, ByteArray(64), byteArrayOf(1)) }
        assertFailsWith<IllegalArgumentException> { recovery.authenticate(challenge, ByteArray(63), byteArrayOf(1)) }
        assertEquals(listOf(9), endpoint.phases)
    }

    @Test fun `changed challenge revokes the native owner instead of renewing its deadline`() {
        val endpoint = Endpoint(); val recovery = open(endpoint).recoveredEnrollment(); recovery.begin()
        endpoint.changed = true
        assertFailsWith<IllegalStateException> { recovery.begin() }
        assertEquals(1, endpoint.closed)
        assertFailsWith<IllegalStateException> { recovery.begin() }
    }

    @Test fun `native signing hash device nonce and original read command must bind the full original challenge`() {
        for (index in listOf(2, 3, 4)) {
            val endpoint = Endpoint().apply { corrupt = index }
            val recovery = open(endpoint).recoveredEnrollment()
            assertFailsWith<IllegalArgumentException> { recovery.begin() }
            assertEquals(1, endpoint.closed)
            assertFailsWith<IllegalStateException> { recovery.begin() }
        }
    }

    @Test fun `lost proof response and wrong native attempt identity revoke rather than retry`() {
        for (wrong in listOf(false, true)) {
            val endpoint = Endpoint().apply { loseProof = !wrong; wrongAttempt = wrong }
            val recovery = open(endpoint).recoveredEnrollment(); val challenge = recovery.begin()
            if (wrong) assertFailsWith<IllegalArgumentException> { recovery.authenticate(challenge, ByteArray(64) { 1 }, byteArrayOf(2)) }
            else assertFailsWith<IllegalStateException> { recovery.authenticate(challenge, ByteArray(64) { 1 }, byteArrayOf(2)) }
            assertEquals(1, endpoint.closed)
            assertFailsWith<IllegalStateException> { recovery.authenticate(challenge, ByteArray(64) { 1 }, byteArrayOf(2)) }
            assertEquals(listOf(9, 10), endpoint.phases)
        }
    }

    @Test fun `close retires outstanding recovery before signatures or hardware can be adopted`() {
        val endpoint = Endpoint(); val native = open(endpoint); val recovery = native.recoveredEnrollment(); val challenge = recovery.begin()
        native.close()
        assertFailsWith<IllegalStateException> { recovery.authenticate(challenge, ByteArray(64), byteArrayOf(1)) }
        assertEquals(listOf(9), endpoint.phases)
        assertEquals(1, endpoint.closed)
    }

    private fun open(endpoint: Endpoint) = KagemushaNativeCoreCoordinatorAdapterV1.openEndpoint("/durable/recovered", endpoint)
    private class Endpoint : KagemushaCoreCoordinatorEndpointV1 {
        val phases = mutableListOf<Int>(); var closed = 0; var changed = false; var corrupt = -1
        var loseProof = false; var wrongAttempt = false
        override fun contract() = intArrayOf(2, 25, 3, 6, 54, 8, 7, 22, 16, 0xffff, 1, 21)
        override fun install(storagePath: String) = 0
        override fun open(storagePath: String) = 31L
        override fun close(handle: Long): Int { closed++; return 0 }
        override fun invoke(handle: Long, method: Int, fields: Array<ByteArray>): Array<ByteArray>? {
            assertEquals(12, method)
            val phase = ByteBuffer.wrap(fields[0]).order(ByteOrder.LITTLE_ENDIAN).int; phases += phase
            return when (phase) {
                9 -> {
                    val raw = fixture("recovery_challenge_canonical_hex")
                    val challenge = KagemushaEnrolledOpenChallengeCodecV1.decodeAccountChallengeShapeExact(raw)
                    arrayOf(u64(if (changed) 8 else 7), raw, fixture("recovery_account_signing_message_hex"),
                        KagemushaDeviceOperationCodecV1.encodeControlCommand(KagemushaDeviceControlCommandV1.ReadActiveHardwareCredential),
                        challenge.nonce()).also { if (corrupt >= 0) it[corrupt][0] = (it[corrupt][0].toInt() xor 1).toByte() }
                }
                10 -> if (loseProof) null else arrayOf(if (wrongAttempt) u64(99) else fields[1].copyOf())
                11 -> emptyArray()
                else -> error("Recovery must never invoke initial enrollment")
            }
        }
    }
    private companion object {
        fun u64(value: Long) = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(value).array()
        fun fixture(field: String): ByteArray {
            val text = String(Files.readAllBytes(Paths.get("../../fixtures/offline/kagemusha_enrolled_open_challenge_v1.json")), StandardCharsets.UTF_8)
            val value = Regex("\"$field\"\\s*:\\s*\"([^\"]+)\"").find(text)!!.groupValues[1]
            return value.chunked(2).map { it.toInt(16).toByte() }.toByteArray()
        }
    }
}
