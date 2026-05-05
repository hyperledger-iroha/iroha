package org.hyperledger.iroha.sdk.core.model.zk

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotEquals
import kotlin.test.assertNull

class VerifyingKeyRecordDescriptionTest {

    private val validSchemaHash = "a".repeat(64)
    private val validCommitment = "b".repeat(64)
    private val backend = "test-backend"
    private val inlineBytes = byteArrayOf(1, 2, 3, 4)

    private fun createWithInlineBytes(
        version: Int = 1,
        circuitId: String = "circuit-1",
        schemaHashHex: String = validSchemaHash,
        gasScheduleId: String = "gas-1",
        inlineKeyBytes: ByteArray = inlineBytes,
    ): VerifyingKeyRecordDescription = VerifyingKeyRecordDescription.create(
        backend = backend,
        version = version,
        circuitId = circuitId,
        schemaHashHex = schemaHashHex,
        gasScheduleId = gasScheduleId,
        inlineKeyBytes = inlineKeyBytes,
    )

    private fun createWithCommitment(
        version: Int = 1,
        circuitId: String = "circuit-1",
        schemaHashHex: String = validSchemaHash,
        gasScheduleId: String = "gas-1",
        commitmentHex: String = validCommitment,
        vkLength: Int = 128,
    ): VerifyingKeyRecordDescription = VerifyingKeyRecordDescription.create(
        backend = backend,
        version = version,
        circuitId = circuitId,
        schemaHashHex = schemaHashHex,
        gasScheduleId = gasScheduleId,
        commitmentHex = commitmentHex,
        vkLength = vkLength,
    )

    // --- Constructor validation ---

    @Test
    fun `create throws on negative version`() {
        assertFailsWith<IllegalArgumentException> {
            createWithInlineBytes(version = -1)
        }
    }

    @Test
    fun `create throws on blank circuitId`() {
        assertFailsWith<IllegalArgumentException> {
            createWithInlineBytes(circuitId = "  ")
        }
    }

    @Test
    fun `create trims circuitId`() {
        val desc = createWithInlineBytes(circuitId = "  circuit-1  ")
        assertEquals("circuit-1", desc.circuitId)
    }

    @Test
    fun `create throws on invalid schemaHashHex length`() {
        assertFailsWith<IllegalArgumentException> {
            createWithInlineBytes(schemaHashHex = "abcdef")
        }
    }

    @Test
    fun `create throws on non-hex schemaHashHex`() {
        assertFailsWith<IllegalArgumentException> {
            createWithInlineBytes(schemaHashHex = "z".repeat(64))
        }
    }

    @Test
    fun `create throws on blank gasScheduleId`() {
        assertFailsWith<IllegalArgumentException> {
            createWithInlineBytes(gasScheduleId = "  ")
        }
    }

    @Test
    fun `create throws when vkLength is zero`() {
        assertFailsWith<IllegalArgumentException> {
            VerifyingKeyRecordDescription.create(
                backend = backend,
                version = 1,
                circuitId = "c",
                schemaHashHex = validSchemaHash,
                gasScheduleId = "g",
                commitmentHex = validCommitment,
                vkLength = 0,
            )
        }
    }

    @Test
    fun `create throws when vkLength is negative`() {
        assertFailsWith<IllegalArgumentException> {
            VerifyingKeyRecordDescription.create(
                backend = backend,
                version = 1,
                circuitId = "c",
                schemaHashHex = validSchemaHash,
                gasScheduleId = "g",
                commitmentHex = validCommitment,
                vkLength = -1,
            )
        }
    }

    @Test
    fun `create throws when maxProofBytes is negative`() {
        assertFailsWith<IllegalArgumentException> {
            VerifyingKeyRecordDescription.create(
                backend = backend,
                version = 1,
                circuitId = "c",
                schemaHashHex = validSchemaHash,
                gasScheduleId = "g",
                inlineKeyBytes = inlineBytes,
                maxProofBytes = -1,
            )
        }
    }

    @Test
    fun `create throws when activationHeight is negative`() {
        assertFailsWith<IllegalArgumentException> {
            VerifyingKeyRecordDescription.create(
                backend = backend,
                version = 1,
                circuitId = "c",
                schemaHashHex = validSchemaHash,
                gasScheduleId = "g",
                inlineKeyBytes = inlineBytes,
                activationHeight = -1,
            )
        }
    }

    @Test
    fun `create throws when withdrawHeight is negative`() {
        assertFailsWith<IllegalArgumentException> {
            VerifyingKeyRecordDescription.create(
                backend = backend,
                version = 1,
                circuitId = "c",
                schemaHashHex = validSchemaHash,
                gasScheduleId = "g",
                inlineKeyBytes = inlineBytes,
                withdrawHeight = -1,
            )
        }
    }

    @Test
    fun `create throws when withdrawHeight less than activationHeight`() {
        assertFailsWith<IllegalStateException> {
            VerifyingKeyRecordDescription.create(
                backend = backend,
                version = 1,
                circuitId = "c",
                schemaHashHex = validSchemaHash,
                gasScheduleId = "g",
                inlineKeyBytes = inlineBytes,
                activationHeight = 100,
                withdrawHeight = 50,
            )
        }
    }

    // --- Cross-field validation ---

    @Test
    fun `create throws when no inline bytes and no commitmentHex`() {
        assertFailsWith<IllegalStateException> {
            VerifyingKeyRecordDescription.create(
                backend = backend,
                version = 1,
                circuitId = "c",
                schemaHashHex = validSchemaHash,
                gasScheduleId = "g",
                vkLength = 128,
            )
        }
    }

    @Test
    fun `create throws when no inline bytes and no vkLength`() {
        assertFailsWith<IllegalStateException> {
            VerifyingKeyRecordDescription.create(
                backend = backend,
                version = 1,
                circuitId = "c",
                schemaHashHex = validSchemaHash,
                gasScheduleId = "g",
                commitmentHex = validCommitment,
            )
        }
    }

    @Test
    fun `create throws when vkLength does not match inline bytes size`() {
        assertFailsWith<IllegalStateException> {
            VerifyingKeyRecordDescription.create(
                backend = backend,
                version = 1,
                circuitId = "c",
                schemaHashHex = validSchemaHash,
                gasScheduleId = "g",
                inlineKeyBytes = inlineBytes,
                vkLength = 999,
            )
        }
    }

    @Test
    fun `create sets vkLength to inline bytes size when not explicitly provided`() {
        val desc = createWithInlineBytes()
        assertEquals(inlineBytes.size, desc.vkLength)
    }

    // --- Defensive copy ---

    @Test
    fun `mutating original array does not affect instance`() {
        val original = byteArrayOf(1, 2, 3)
        val desc = createWithInlineBytes(inlineKeyBytes = original)
        original[0] = 99
        assertEquals(1, desc.inlineKeyBytes!![0])
    }

    @Test
    fun `mutating returned array does not affect instance`() {
        val desc = createWithInlineBytes()
        desc.inlineKeyBytes!![0] = 99
        assertEquals(inlineBytes[0], desc.inlineKeyBytes!![0])
    }

    @Test
    fun `inlineKeyBytes returns null when not provided`() {
        val desc = createWithCommitment()
        assertNull(desc.inlineKeyBytes)
    }

    // --- Equality & hashing ---

    @Test
    fun `equal instances have same hashCode`() {
        val a = createWithCommitment()
        val b = createWithCommitment()
        assertEquals(a, b)
        assertEquals(a.hashCode(), b.hashCode())
    }

    @Test
    fun `different instances are not equal`() {
        val a = createWithCommitment(version = 1)
        val b = createWithCommitment(version = 2)
        assertNotEquals(a, b)
    }

    @Test
    fun `instance does not equal null`() {
        val desc = createWithCommitment()
        assertNotEquals<Any?>(desc, null)
    }

    // --- Factory with defaults ---

    @Test
    fun `default backendTag is UNSUPPORTED`() {
        val desc = createWithInlineBytes()
        assertEquals(VerifyingKeyBackendTag.UNSUPPORTED, desc.backendTag)
    }

    @Test
    fun `default curve is unknown`() {
        val desc = createWithInlineBytes()
        assertEquals("unknown", desc.curve)
    }

    @Test
    fun `default status is ACTIVE`() {
        val desc = createWithInlineBytes()
        assertEquals(VerifyingKeyStatus.ACTIVE, desc.status)
    }

    // --- toArguments ---

    @Test
    fun `toArguments includes required fields`() {
        val desc = createWithCommitment()
        val args = desc.toArguments(backend)
        assertEquals("1", args["record.version"])
        assertEquals("circuit-1", args["record.circuit_id"])
        assertEquals(VerifyingKeyBackendTag.UNSUPPORTED.noritoValue, args["record.backend_tag"])
        assertEquals("gas-1", args["record.gas_schedule_id"])
        assertEquals(VerifyingKeyStatus.ACTIVE.wireName, args["record.status"])
    }

    @Test
    fun `toArguments omits null optional fields`() {
        val desc = createWithCommitment()
        val args = desc.toArguments(backend)
        assertNull(args["record.metadata_uri_cid"])
        assertNull(args["record.vk_bytes_cid"])
        assertNull(args["record.activation_height"])
        assertNull(args["record.withdraw_height"])
    }

    @Test
    fun `toArguments includes optional fields when set`() {
        val desc = VerifyingKeyRecordDescription.create(
            backend = backend,
            version = 1,
            circuitId = "c",
            schemaHashHex = validSchemaHash,
            gasScheduleId = "g",
            commitmentHex = validCommitment,
            vkLength = 128,
            metadataUriCid = "meta-cid",
            vkBytesCid = "vk-cid",
            activationHeight = 10,
            withdrawHeight = 20,
        )
        val args = desc.toArguments(backend)
        assertEquals("meta-cid", args["record.metadata_uri_cid"])
        assertEquals("vk-cid", args["record.vk_bytes_cid"])
        assertEquals("10", args["record.activation_height"])
        assertEquals("20", args["record.withdraw_height"])
    }
}
