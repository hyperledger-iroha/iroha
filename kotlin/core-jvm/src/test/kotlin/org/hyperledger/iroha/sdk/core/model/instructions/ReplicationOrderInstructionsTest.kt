@file:OptIn(kotlin.io.encoding.ExperimentalEncodingApi::class)

package org.hyperledger.iroha.sdk.core.model.instructions

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class ReplicationOrderInstructionsTest {
    private val orderId = "44".repeat(32)

    @Test
    fun `issue arguments are canonical and roundtrip`() {
        val instruction = IssueReplicationOrderInstruction(orderId, "AQID", 20, 28)
        assertEquals("IssueReplicationOrder", instruction.arguments["action"])
        assertEquals(instruction, IssueReplicationOrderInstruction.fromArguments(instruction.arguments))
    }

    @Test
    fun `issue rejects malformed identifiers payloads and windows`() {
        assertFailsWith<IllegalArgumentException> {
            IssueReplicationOrderInstruction("AA".repeat(32), "AQID", 20, 28)
        }
        assertFailsWith<IllegalArgumentException> {
            IssueReplicationOrderInstruction("00".repeat(32), "AQID", 20, 28)
        }
        assertFailsWith<IllegalArgumentException> {
            IssueReplicationOrderInstruction(orderId, "AQID\n", 20, 28)
        }
        assertFailsWith<IllegalArgumentException> {
            IssueReplicationOrderInstruction(orderId, "AQID", 20, 20)
        }
        assertFailsWith<IllegalArgumentException> {
            IssueReplicationOrderInstruction(orderId, "AQID", 29, 28)
        }
    }

    @Test
    fun `raw order id conversion is fixed width and bounded`() {
        val bytes = ByteArray(32) { 0x80.toByte() }
        val instruction = IssueReplicationOrderInstruction.fromOrderBytes(bytes, byteArrayOf(1), 1, 2)
        assertEquals("80".repeat(32), instruction.orderIdHex)
        assertFailsWith<IllegalArgumentException> {
            IssueReplicationOrderInstruction.fromOrderBytes(ByteArray(31) { 1 }, byteArrayOf(1), 1, 2)
        }
        assertFailsWith<IllegalArgumentException> {
            IssueReplicationOrderInstruction.fromOrderBytes(ByteArray(32), byteArrayOf(1), 1, 2)
        }
    }

    @Test
    fun `issue rejects oversized decoded payload`() {
        val oversized = kotlin.io.encoding.Base64.encode(ByteArray(1024 * 1024 + 1) { 1 })
        assertFailsWith<IllegalArgumentException> {
            IssueReplicationOrderInstruction(orderId, oversized, 1, 2)
        }
    }

    @Test
    fun `argument decoders reject confused deputy fields`() {
        val issue = IssueReplicationOrderInstruction(orderId, "AQID", 20, 28)
        assertFailsWith<IllegalArgumentException> {
            IssueReplicationOrderInstruction.fromArguments(issue.arguments + ("unexpected" to "field"))
        }
        assertFailsWith<IllegalArgumentException> {
            IssueReplicationOrderInstruction.fromArguments(
                issue.arguments + ("action" to "CompleteReplicationOrder"),
            )
        }
    }

    @Test
    fun `complete and expire roundtrip and reject malformed epochs`() {
        val providerId = "11".repeat(32)
        val complete = CompleteReplicationOrderInstruction(orderId, providerId, 28)
        assertEquals(complete, CompleteReplicationOrderInstruction.fromArguments(complete.arguments))
        val expire = ExpireReplicationOrderInstruction(orderId, 29)
        assertEquals("ExpireReplicationOrder", expire.arguments["action"])
        assertEquals(expire, ExpireReplicationOrderInstruction.fromArguments(expire.arguments))

        assertFailsWith<IllegalArgumentException> {
            CompleteReplicationOrderInstruction(orderId, providerId, -1)
        }
        assertFailsWith<IllegalArgumentException> {
            CompleteReplicationOrderInstruction(orderId, "00".repeat(32), 28)
        }
        assertFailsWith<IllegalArgumentException> {
            ExpireReplicationOrderInstruction(orderId, -1)
        }
        assertFailsWith<IllegalArgumentException> {
            ExpireReplicationOrderInstruction.fromArguments(expire.arguments + ("extra" to "x"))
        }
    }
}
