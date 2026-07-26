package org.hyperledger.iroha.sdk.core.model.instructions

import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.core.model.WirePayload
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter
import org.hyperledger.iroha.sdk.numeric.KotodamaQuantity

class CancelAssetLockInstructionTest {

    @Test
    fun `builder derives the native escrow id and emits only V1 fields`() {
        val instruction = CancelAssetLockInstruction(
            lockId = FIXTURE_LOCK_ID,
            expectedRemainingAmount = "20",
        )

        assertEquals(FIXTURE_ESCROW_ID, instruction.escrowId.value)
        assertEquals(
            mapOf(
                "escrow_id" to FIXTURE_ESCROW_ID,
                "expected_remaining_amount" to "20",
            ),
            instruction.arguments,
        )
        assertFailsWith<UnsupportedOperationException> {
            @Suppress("UNCHECKED_CAST")
            (instruction.arguments as MutableMap<String, String>)["amount"] = "20"
        }
        val wire = assertIs<WirePayload>(instruction.toInstructionBox().payload)
        assertEquals(CancelAssetLockInstruction.WIRE_NAME, wire.wireName)
        assertContentEquals(CANONICAL_PAYLOAD, wire.payloadBytes)
    }

    @Test
    fun `canonical native fields reject the retired and ambiguous surfaces`() {
        val valid = mapOf(
            "escrow_id" to FIXTURE_ESCROW_ID,
            "expected_remaining_amount" to "20",
        )
        assertEquals(
            CancelAssetLockInstruction(FIXTURE_LOCK_ID, "20"),
            CancelAssetLockInstruction.fromCanonicalFields(valid),
        )

        listOf(
            mapOf("escrow_id" to FIXTURE_ESCROW_ID),
            mapOf(
                "escrow_id" to FIXTURE_ESCROW_ID,
                "expectedRemainingAmount" to "20",
            ),
            valid + ("amount" to "20"),
        ).forEach { fields ->
            assertFailsWith<IllegalArgumentException> {
                CancelAssetLockInstruction.fromCanonicalFields(fields)
            }
        }

        assertFailsWith<IllegalArgumentException> {
            CancelAssetLockInstruction.fromEscrowId(FIXTURE_ESCROW_ID.lowercase(), "20")
        }
    }

    @Test
    fun `expected remaining amount is positive and canonically spelled`() {
        listOf("", " ", "0", "-1", "+20", "020", "20.0", "1e1", "20 ").forEach { amount ->
            assertFailsWith<IllegalArgumentException>("accepted '$amount'") {
                CancelAssetLockInstruction(FIXTURE_LOCK_ID, amount)
            }
            assertFailsWith<IllegalArgumentException>("read back '$amount'") {
                CancelAssetLockInstruction.fromCanonicalFields(
                    mapOf(
                        "escrow_id" to FIXTURE_ESCROW_ID,
                        "expected_remaining_amount" to amount,
                    ),
                )
            }
        }
        assertFailsWith<IllegalArgumentException> {
            CancelAssetLockInstruction(
                FIXTURE_LOCK_ID,
                KotodamaQuantity.of(java.math.BigInteger.ZERO, 0),
            )
        }
        listOf(
            "",
            " ",
            " $FIXTURE_LOCK_ID",
            "$FIXTURE_LOCK_ID ",
            "\uFEFF$FIXTURE_LOCK_ID",
            "$FIXTURE_LOCK_ID\uFEFF",
        ).forEach {
            assertFailsWith<IllegalArgumentException>("accepted lock id '$it'") {
                CancelAssetLockInstruction(it, "20")
            }
        }
    }

    @Test
    fun `lock id preimage uses the exact UTF-8 byte bound`() {
        val exactBound = "🔒".repeat(1_024)
        assertEquals(4_096, exactBound.toByteArray(Charsets.UTF_8).size)
        assertEquals(
            4_096,
            CancelAssetLockInstruction.MAX_LOCK_ID_UTF8_BYTES_V1,
        )
        CancelAssetLockInstruction(exactBound, "1")

        val overBound = exactBound + "a"
        assertEquals(4_097, overBound.toByteArray(Charsets.UTF_8).size)
        assertFailsWith<IllegalArgumentException> {
            CancelAssetLockInstruction(overBound, "1")
        }
    }

    @Test
    fun `wire decoder is strict and roundtrips the canonical frame`() {
        assertEquals(85, CANONICAL_PAYLOAD.size)
        val decoded = CancelAssetLockInstruction.fromWirePayload(CANONICAL_PAYLOAD)
        assertEquals(FIXTURE_ESCROW_ID, decoded.escrowId.value)
        assertEquals("20", decoded.expectedRemainingAmount.toString())
        assertContentEquals(
            CANONICAL_PAYLOAD,
            CancelAssetLockWirePayloadEncoder.encodePayload(decoded),
        )

        val trailing = CANONICAL_PAYLOAD + byteArrayOf(0)
        assertFailsWith<IllegalArgumentException> {
            CancelAssetLockInstruction.fromWirePayload(trailing)
        }
        assertFailsWith<IllegalArgumentException> {
            CancelAssetLockInstruction.fromWirePayload(legacyOneFieldFrame())
        }
        assertEquals(86, RETIRED_NESTED_ESCROW_ID_PAYLOAD.size)
        assertContentEquals(
            byteArrayOf(0x21, 0x20),
            RETIRED_NESTED_ESCROW_ID_PAYLOAD.copyOfRange(40, 42),
        )
        assertFailsWith<IllegalArgumentException> {
            CancelAssetLockInstruction.fromWirePayload(RETIRED_NESTED_ESCROW_ID_PAYLOAD)
        }
    }

    @Test
    fun `checked in appeal finance fixtures are mandatory and byte exact`() {
        val root = requireFixtureRoot()
        val fixtures = REQUIRED_FIXTURE_NAMES.associateWith { relative ->
            readMandatoryFixture(root, relative)
        }
        assertEquals(8, fixtures.size)
        assertTrue(fixtures.values.all(ByteArray::isNotEmpty))
        assertContentEquals(
            fixtures.getValue("cancel_asset_lock_v1.to"),
            CancelAssetLockWirePayloadEncoder.encodePayload(
                CancelAssetLockInstruction(FIXTURE_LOCK_ID, "20"),
            ),
        )
        assertContentEquals(
            RETIRED_NESTED_ESCROW_ID_PAYLOAD,
            fixtures.getValue("negative/cancel_asset_lock_nested_escrow_id_v1.to"),
        )
        listOf(
            "negative/cancel_asset_lock_legacy_missing_expected_v1.to",
            "negative/cancel_asset_lock_nested_escrow_id_v1.to",
            "negative/cancel_asset_lock_zero_expected_v1.to",
        ).forEach { relative ->
            assertFailsWith<IllegalArgumentException>("accepted $relative") {
                CancelAssetLockInstruction.fromWirePayload(
                    fixtures.getValue(relative),
                )
            }
        }
    }

    private fun legacyOneFieldFrame(): ByteArray =
        NoritoCodec.encode(
            cancelAssetLockTestHexToBytes(FIXTURE_ESCROW_ID.substring(5, 69)),
            CancelAssetLockInstruction.WIRE_NAME,
            object : TypeAdapter<ByteArray> {
                override fun encode(
                    encoder: NoritoEncoder,
                    value: ByteArray,
                ) {
                    encoder.writeLength(
                        value.size.toLong(),
                        (encoder.flags and NoritoHeader.COMPACT_LEN) != 0,
                    )
                    encoder.writeBytes(value)
                }

                override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): ByteArray =
                    throw UnsupportedOperationException()
            },
        )

    private fun requireFixtureRoot(): Path {
        val candidates = listOf(
            Paths.get("../../fixtures/sorafs_manifest/appeal_finance"),
            Paths.get("../fixtures/sorafs_manifest/appeal_finance"),
            Paths.get("fixtures/sorafs_manifest/appeal_finance"),
        )
        return candidates.firstOrNull { Files.isDirectory(it) }
            ?: error(
                "Missing mandatory CancelAssetLock fixture directory; searched: " +
                    candidates.joinToString(),
            )
    }

    private fun readMandatoryFixture(
        root: Path,
        relative: String,
    ): ByteArray {
        val path = root.resolve(relative)
        check(Files.isRegularFile(path)) {
            "Missing mandatory CancelAssetLock fixture `$relative` at $path"
        }
        return Files.readAllBytes(path)
    }

    private companion object {
        const val FIXTURE_LOCK_ID = "sorafs-appeal-cancel-asset-lock-v1"
        const val FIXTURE_ESCROW_ID =
            "hash:73CCD4E0DD69AD434DB75056B600AA4F74C8FC5556B11BDC799DFDB7EA29851F#434B"
        val REQUIRED_FIXTURE_NAMES = listOf(
            "cancel_asset_lock_v1.json",
            "cancel_asset_lock_v1.to",
            "negative/cancel_asset_lock_legacy_missing_expected_v1.json",
            "negative/cancel_asset_lock_legacy_missing_expected_v1.to",
            "negative/cancel_asset_lock_nested_escrow_id_v1.to",
            "negative/cancel_asset_lock_noncanonical_quantity_v1.json",
            "negative/cancel_asset_lock_zero_expected_v1.json",
            "negative/cancel_asset_lock_zero_expected_v1.to",
        )
        val CANONICAL_PAYLOAD = cancelAssetLockTestHexToBytes(
            "4e5254300000b5c8a665a7de80e2eef75ccb287078fa002d00000000000000" +
                "d5f0a9bf0af707a1022073ccd4e0dd69ad434db75056b600aa4f74c8fc5556b11bdc" +
                "799dfdb7ea29851f0b0501000000140400000000",
        )
        val RETIRED_NESTED_ESCROW_ID_PAYLOAD = cancelAssetLockTestHexToBytes(
            "4e5254300000b5c8a665a7de80e2eef75ccb287078fa002e00000000000000" +
                "0e55fb7ed463b87302212073ccd4e0dd69ad434db75056b600aa4f74c8fc5556b11b" +
                "dc799dfdb7ea29851f0b0501000000140400000000",
        )
    }
}

private fun cancelAssetLockTestHexToBytes(value: String): ByteArray =
    ByteArray(value.length / 2) { index ->
        value.substring(index * 2, index * 2 + 2).toInt(16).toByte()
    }
