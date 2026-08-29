// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.privacy

import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash

class PrivacyCompiledProfileCatalogV1Test {
    @Test
    fun permanentSchemaAndAllAvailableRowsRoundTripCanonically() {
        assertContentEquals(
            SchemaHash.hash16(PrivacyCompiledProfileCatalogCodecV1.SCHEMA_NAME),
            PrivacyCompiledProfileCatalogCodecV1.schemaHashV1(),
        )
        val catalog = availableCatalog()
        val archive = PrivacyCompiledProfileCatalogCodecV1.encodeCanonical(catalog)
        val decoded = PrivacyCompiledProfileCatalogCodecV1.decodeCanonical(archive)
        assertEquals(catalog, decoded)
        assertEquals(PrivacyProtocolIdV1.values().toList(), decoded.protocols.map { it.protocolId })
        decoded.protocols.forEach { row ->
            val profile = assertIs<PrivacyCompiledProfileResultV1.Available>(row.compiledProfile).profile
            assertEquals(row.protocolId.expectedProofSystem, profile.proofSystemId)
            assertEquals(row.protocolId.expectedEngine, profile.engineId)
            assertEquals(row.protocolId, profile.protocolLimits.protocolId)
        }
    }

    @Test
    fun everyUnavailableReasonRoundTripsWithoutInventingReadiness() {
        val reasons = listOf(
            PrivacyCompiledProfileResultV1.Unavailable(
                PrivacyCompiledProfileUnavailableReasonV1.ENGINE_UNAVAILABLE,
                null,
            ),
            PrivacyCompiledProfileResultV1.Unavailable(
                PrivacyCompiledProfileUnavailableReasonV1.PROFILE_INITIALIZATION_FAILED,
                null,
            ),
            PrivacyCompiledProfileResultV1.Unavailable(
                PrivacyCompiledProfileUnavailableReasonV1.STATEMENT_SCHEMA_INVALID,
                PrivacyCompiledStatementSchemaErrorV1.CONFLICTING_STABLE_TYPE_ID,
            ),
            PrivacyCompiledProfileResultV1.Unavailable(
                PrivacyCompiledProfileUnavailableReasonV1.STATEMENT_SCHEMA_INVALID,
                PrivacyCompiledStatementSchemaErrorV1.MISSING_TYPE_REFERENCE,
            ),
        )
        val catalog = PrivacyCompiledProfileCatalogV1(
            1,
            PrivacyProtocolIdV1.values().mapIndexed { index, protocol ->
                PrivacyCompiledProfileCatalogRowV1(protocol, reasons[index % reasons.size])
            },
        )
        val decoded = PrivacyCompiledProfileCatalogCodecV1.decodeCanonical(
            PrivacyCompiledProfileCatalogCodecV1.encodeCanonical(catalog),
        )
        assertEquals(catalog, decoded)
        assertTrue(decoded.protocols.none { it.compiledProfile is PrivacyCompiledProfileResultV1.Available })
    }

    @Test
    fun rejectsHeaderFramingAndCanonicalityAdversaries() {
        val canonical = PrivacyCompiledProfileCatalogCodecV1.encodeCanonical(availableCatalog())
        val badMagic = canonical.copyOf().also { it[0] = (it[0].toInt() xor 0x80).toByte() }
        val wrongSchema = canonical.copyOf().also { it[6] = (it[6].toInt() xor 0x80).toByte() }
        val badChecksum = canonical.copyOf().also { it[31] = (it[31].toInt() xor 0x80).toByte() }
        val compressed = canonical.copyOf().also { it[22] = NoritoHeader.COMPRESSION_ZSTD.toByte() }
        val wrongFlags = canonical.copyOf().also { it[39] = 0 }
        val wrongLength = canonical.copyOf().also {
            ByteBuffer.wrap(it).order(ByteOrder.LITTLE_ENDIAN).putLong(23, 1L)
        }
        val overlongVersionLength = overlongFirstLength(canonical)

        listOf(
            byteArrayOf(),
            canonical.copyOfRange(0, NoritoHeader.HEADER_LENGTH - 1),
            canonical.copyOfRange(0, canonical.size - 1),
            canonical + byteArrayOf(0),
            badMagic,
            wrongSchema,
            badChecksum,
            compressed,
            wrongFlags,
            wrongLength,
            overlongVersionLength,
        ).forEach { hostile ->
            assertFailsWith<IllegalArgumentException> {
                PrivacyCompiledProfileCatalogCodecV1.decodeCanonical(hostile)
            }
        }
    }

    @Test
    fun rejectsUnknownTagsSequenceBombsZeroBindingsAndCrossMappings() {
        val canonical = PrivacyCompiledProfileCatalogCodecV1.encodeCanonical(availableCatalog())
        val offsets = firstAvailableRowOffsets(canonical)

        val unknownProtocol = canonical.copyOf().also {
            ByteBuffer.wrap(it).order(ByteOrder.LITTLE_ENDIAN).putInt(offsets.rowProtocol, 12)
        }.withRecomputedChecksum()
        val unknownResult = canonical.copyOf().also {
            ByteBuffer.wrap(it).order(ByteOrder.LITTLE_ENDIAN).putInt(offsets.resultTag, 2)
        }.withRecomputedChecksum()
        val wrongProofSystem = canonical.copyOf().also {
            ByteBuffer.wrap(it).order(ByteOrder.LITTLE_ENDIAN).putInt(offsets.proofSystem, 1)
        }.withRecomputedChecksum()
        val tooManyRows = canonical.copyOf().also {
            ByteBuffer.wrap(it).order(ByteOrder.LITTLE_ENDIAN).putLong(offsets.rowCount, 13L)
        }.withRecomputedChecksum()
        val zeroBinding = canonical.copyOf().also {
            for (index in offsets.firstBinding until offsets.firstBinding + 32) it[index] = 0
        }.withRecomputedChecksum()

        listOf(unknownProtocol, unknownResult, wrongProofSystem, tooManyRows, zeroBinding).forEach {
            assertFailsWith<IllegalArgumentException> {
                PrivacyCompiledProfileCatalogCodecV1.decodeCanonical(it)
            }
        }
    }

    @Test
    fun immutableModelsRejectInvalidConstructionBeforeEncoding() {
        val protocols = availableCatalog().protocols.toMutableList()
        assertFailsWith<IllegalArgumentException> {
            PrivacyCompiledProfileCatalogV1(2, protocols)
        }
        assertFailsWith<IllegalArgumentException> {
            PrivacyCompiledProfileCatalogV1(1, protocols.dropLast(1))
        }
        assertFailsWith<IllegalArgumentException> {
            PrivacyCompiledProfileCatalogV1(1, protocols.reversed())
        }
        assertFailsWith<UnsupportedOperationException> {
            @Suppress("UNCHECKED_CAST")
            (availableCatalog().protocols as MutableList<PrivacyCompiledProfileCatalogRowV1>).clear()
        }
        assertFailsWith<IllegalArgumentException> {
            PrivacyProtocolLimitsV1(
                PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1,
                mapOf("max_anonymity_set_size" to 63, "max_recipient_count" to 8),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            PrivacyCompiledProfileResultV1.Unavailable(
                PrivacyCompiledProfileUnavailableReasonV1.ENGINE_UNAVAILABLE,
                PrivacyCompiledStatementSchemaErrorV1.MISSING_TYPE_REFERENCE,
            )
        }
    }

    private fun availableCatalog(): PrivacyCompiledProfileCatalogV1 =
        PrivacyCompiledProfileCatalogV1(
            1,
            PrivacyProtocolIdV1.values().map { protocol ->
                PrivacyCompiledProfileCatalogRowV1(
                    protocol,
                    PrivacyCompiledProfileResultV1.Available(profile(protocol)),
                )
            },
        )

    private fun profile(protocol: PrivacyProtocolIdV1): PrivacyCompiledProfileV1 {
        fun binding(offset: Int): PrivacyFixed32V1 =
            PrivacyFixed32V1(ByteArray(32) { (protocol.ordinal * 7 + offset).toByte() })
        return PrivacyCompiledProfileV1(
            protocol,
            protocol.expectedProofSystem,
            protocol.expectedEngine,
            binding(1),
            binding(2),
            binding(3),
            binding(4),
            binding(5),
            limits(protocol),
        )
    }

    private fun limits(protocol: PrivacyProtocolIdV1): PrivacyProtocolLimitsV1 =
        PrivacyProtocolLimitsV1(
            protocol,
            when (protocol) {
                PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1 ->
                    mapOf("max_anonymity_set_size" to 64, "max_recipient_count" to 8)
                PrivacyProtocolIdV1.VERANGE_TRANSPARENT_RANGE_V1 ->
                    mapOf("max_aggregation_count" to 8)
                PrivacyProtocolIdV1.IROHA_ZK_AMS_V1 ->
                    mapOf("max_batch_size" to 8, "max_ring_size" to 64)
                PrivacyProtocolIdV1.IROHA_JINDO_POLYNOMIAL_COMMITMENT_V1 ->
                    mapOf("max_polynomial_count" to 4)
                PrivacyProtocolIdV1.ORCHARD_HALO2_ACTIONS_V1 ->
                    mapOf("max_action_count" to 2)
                PrivacyProtocolIdV1.MONERO_FCMP_PLUS_PLUS_V1 ->
                    mapOf("max_input_count" to 2, "max_output_count" to 4)
                PrivacyProtocolIdV1.IROHA_IVM_PRIVATE_NOTE_STARK_V1,
                PrivacyProtocolIdV1.PQ_MASP_STARK_V1,
                -> mapOf("max_input_count" to 2, "max_output_count" to 2)
                else -> null
            },
        )

    private data class FirstRowOffsets(
        val rowCount: Int,
        val rowProtocol: Int,
        val resultTag: Int,
        val proofSystem: Int,
        val firstBinding: Int,
    )

    private fun firstAvailableRowOffsets(archive: ByteArray): FirstRowOffsets {
        val cursor = Cursor(archive, NoritoHeader.HEADER_LENGTH)
        cursor.skipSizedField()
        cursor.readVarint()
        val rowCount = cursor.position
        cursor.position += 8
        cursor.readVarint()
        cursor.readVarint()
        val rowProtocol = cursor.position
        cursor.position += 4
        cursor.readVarint()
        val resultTag = cursor.position
        cursor.position += 4
        cursor.readVarint()
        cursor.skipSizedField()
        cursor.readVarint()
        val proofSystem = cursor.position
        cursor.position += 4
        cursor.skipSizedField()
        cursor.readVarint()
        cursor.readVarint()
        val firstBinding = cursor.position
        return FirstRowOffsets(rowCount, rowProtocol, resultTag, proofSystem, firstBinding)
    }

    private fun overlongFirstLength(canonical: ByteArray): ByteArray {
        val result = ByteArray(canonical.size + 1)
        canonical.copyInto(result, endIndex = NoritoHeader.HEADER_LENGTH)
        result[NoritoHeader.HEADER_LENGTH] = 0x84.toByte()
        result[NoritoHeader.HEADER_LENGTH + 1] = 0
        canonical.copyInto(
            result,
            destinationOffset = NoritoHeader.HEADER_LENGTH + 2,
            startIndex = NoritoHeader.HEADER_LENGTH + 1,
        )
        ByteBuffer.wrap(result).order(ByteOrder.LITTLE_ENDIAN).putLong(
            23,
            (result.size - NoritoHeader.HEADER_LENGTH).toLong(),
        )
        return result.withRecomputedChecksum()
    }

    private fun ByteArray.withRecomputedChecksum(): ByteArray = apply {
        val payload = copyOfRange(NoritoHeader.HEADER_LENGTH, size)
        ByteBuffer.wrap(this).order(ByteOrder.LITTLE_ENDIAN).putLong(31, CRC64.compute(payload))
    }

    private class Cursor(
        private val bytes: ByteArray,
        var position: Int,
    ) {
        fun readVarint(): Long {
            var value = 0L
            var shift = 0
            while (true) {
                val octet = bytes[position++].toInt() and 0xff
                value = value or ((octet and 0x7f).toLong() shl shift)
                if (octet and 0x80 == 0) return value
                shift += 7
            }
        }

        fun skipSizedField() {
            position += readVarint().toInt()
        }
    }
}
