// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.privacy

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.Collections
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter

/** One local build result in the closed first-release protocol order. */
class PrivacyCompiledProfileCatalogRowV1(
    @JvmField val protocolId: PrivacyProtocolIdV1,
    @JvmField val compiledProfile: PrivacyCompiledProfileResultV1,
) {
    init {
        val available = compiledProfile as? PrivacyCompiledProfileResultV1.Available
        require(available == null || available.profile.protocolId == protocolId) {
            "compiled-profile catalog row does not match its embedded protocol"
        }
    }

    override fun equals(other: Any?): Boolean =
        other is PrivacyCompiledProfileCatalogRowV1 &&
            protocolId == other.protocolId &&
            compiledProfile == other.compiledProfile

    override fun hashCode(): Int = 31 * protocolId.hashCode() + compiledProfile.hashCode()
}

/** Canonical local build metadata; this is not authoritative network activation state. */
class PrivacyCompiledProfileCatalogV1(
    @JvmField val version: Int,
    protocols: List<PrivacyCompiledProfileCatalogRowV1>,
) {
    @JvmField
    val protocols: List<PrivacyCompiledProfileCatalogRowV1>

    init {
        require(version == PrivacyCompiledProfileCatalogCodecV1.VERSION) {
            "compiled-profile catalog version must be ${PrivacyCompiledProfileCatalogCodecV1.VERSION}"
        }
        val expected = PrivacyProtocolIdV1.values()
        require(protocols.size == expected.size) {
            "compiled-profile catalog must contain exactly ${expected.size} rows"
        }
        protocols.forEachIndexed { index, row ->
            require(row.protocolId == expected[index]) {
                "compiled-profile catalog row $index is out of canonical protocol order"
            }
        }
        this.protocols = Collections.unmodifiableList(protocols.toList())
    }

    override fun equals(other: Any?): Boolean =
        other is PrivacyCompiledProfileCatalogV1 &&
            version == other.version &&
            protocols == other.protocols

    override fun hashCode(): Int = 31 * version + protocols.hashCode()
}

/**
 * Native-independent canonical Norito codec for `PrivacyCompiledProfileCatalogV1`.
 *
 * The schema hash is derived from the permanent public V1 schema name. The codec accepts
 * only the uncompressed compact-length representation and re-encodes before returning, so
 * alternate varints, field framing, ordering, padding, and trailing data fail closed.
 */
object PrivacyCompiledProfileCatalogCodecV1 {
    const val VERSION: Int = 1
    const val ROW_COUNT: Int = 12
    const val MAX_ARCHIVE_BYTES: Int = 256 * 1024
    const val SCHEMA_NAME: String = "iroha.privacy.compiled-profile-catalog.v1"
    const val SCHEMA_HASH_HEX: String = "f3addcc8d28f55b9119e6cc22e5e5b57"

    private const val HEADER_COMPRESSION_OFFSET: Int = 22
    private const val HEADER_PAYLOAD_LENGTH_OFFSET: Int = 23
    private const val HEADER_FLAGS_OFFSET: Int = NoritoHeader.HEADER_LENGTH - 1
    private const val MAX_ROW_ENCODED_BYTES: Long = 4 * 1024L
    private const val MAX_PROFILE_RESULT_ENCODED_BYTES: Long = 2 * 1024L
    private const val MAX_PROFILE_ENCODED_BYTES: Long = 2 * 1024L
    private const val MAX_LIMITS_ENCODED_BYTES: Long = 128L
    private const val FIXED32_WRAPPER_ENCODED_BYTES: Long = 33L
    private val SCHEMA_HASH: ByteArray = decodeHex(SCHEMA_HASH_HEX)
    private val UINT32_ADAPTER: TypeAdapter<Long> = NoritoAdapters.uint(32)

    /** Return a defensive copy of the exact Rust structural-schema binding. */
    @JvmStatic
    fun schemaHashV1(): ByteArray = SCHEMA_HASH.copyOf()

    /** Decode and semantically validate one complete canonical catalog frame. */
    @JvmStatic
    fun decodeCanonical(archive: ByteArray): PrivacyCompiledProfileCatalogV1 {
        require(archive.isNotEmpty()) { "compiled-profile catalog archive must not be empty" }
        require(archive.size <= MAX_ARCHIVE_BYTES) {
            "compiled-profile catalog archive exceeds $MAX_ARCHIVE_BYTES bytes"
        }
        require(archive.size >= NoritoHeader.HEADER_LENGTH) {
            "compiled-profile catalog is truncated before the Norito header"
        }
        val snapshot = archive.copyOf()
        require(
            (snapshot[HEADER_COMPRESSION_OFFSET].toInt() and 0xff) ==
                NoritoHeader.COMPRESSION_NONE,
        ) { "compiled-profile catalog must use uncompressed Norito" }
        require(
            (snapshot[HEADER_FLAGS_OFFSET].toInt() and 0xff) == NoritoHeader.COMPACT_LEN,
        ) { "compiled-profile catalog must use only the canonical compact-length flag" }
        val declaredPayloadLength = ByteBuffer.wrap(snapshot)
            .order(ByteOrder.LITTLE_ENDIAN)
            .getLong(HEADER_PAYLOAD_LENGTH_OFFSET)
        require(
            declaredPayloadLength in
                0L..(MAX_ARCHIVE_BYTES - NoritoHeader.HEADER_LENGTH).toLong(),
        ) { "compiled-profile catalog declares an oversized Norito payload" }
        require(
            declaredPayloadLength == (snapshot.size - NoritoHeader.HEADER_LENGTH).toLong(),
        ) { "compiled-profile catalog payload length does not cover the complete archive" }

        val framed = NoritoHeader.decode(snapshot, SCHEMA_HASH)
        framed.header.validateChecksum(framed.payload)
        val decoder = NoritoDecoder(framed.payload, framed.header.flags, framed.header.minor)
        val catalog = CatalogAdapter.decode(decoder)
        require(decoder.remaining() == 0) {
            "compiled-profile catalog contains trailing payload data"
        }
        require(snapshot.contentEquals(encodeCanonical(catalog))) {
            "compiled-profile catalog is not byte-canonical Norito"
        }
        return catalog
    }

    /** Encode a validated catalog with the exact frozen V1 schema and layout. */
    @JvmStatic
    fun encodeCanonical(catalog: PrivacyCompiledProfileCatalogV1): ByteArray {
        val encoder = NoritoEncoder(NoritoHeader.COMPACT_LEN)
        CatalogAdapter.encode(encoder, catalog)
        val payload = encoder.toByteArray()
        require(payload.size <= MAX_ARCHIVE_BYTES - NoritoHeader.HEADER_LENGTH) {
            "compiled-profile catalog payload exceeds the first-release bound"
        }
        val header = NoritoHeader(
            SCHEMA_HASH,
            payload.size,
            CRC64.compute(payload),
            NoritoHeader.COMPACT_LEN,
            NoritoHeader.COMPRESSION_NONE,
        ).encode()
        return ByteArray(header.size + payload.size).also { archive ->
            header.copyInto(archive)
            payload.copyInto(archive, header.size)
        }
    }

    private object CatalogAdapter : TypeAdapter<PrivacyCompiledProfileCatalogV1> {
        override fun encode(encoder: NoritoEncoder, value: PrivacyCompiledProfileCatalogV1) {
            encodeSizedField(encoder, UINT32_ADAPTER, value.version.toLong())
            encodeSizedField(encoder, RowsAdapter, value.protocols)
        }

        override fun decode(decoder: NoritoDecoder): PrivacyCompiledProfileCatalogV1 {
            val version = decodeExactSizedField(
                decoder,
                UINT32_ADAPTER,
                4L,
                "catalog version",
            ).toInt()
            val rows = decodeBoundedSizedField(
                decoder,
                RowsAdapter,
                (MAX_ARCHIVE_BYTES - NoritoHeader.HEADER_LENGTH).toLong(),
                "catalog rows",
            )
            return PrivacyCompiledProfileCatalogV1(version, rows)
        }
    }

    private object RowsAdapter : TypeAdapter<List<PrivacyCompiledProfileCatalogRowV1>> {
        override fun encode(
            encoder: NoritoEncoder,
            value: List<PrivacyCompiledProfileCatalogRowV1>,
        ) {
            require(value.size == ROW_COUNT) { "compiled-profile catalog row count must be $ROW_COUNT" }
            encoder.writeLength(value.size.toLong(), false)
            value.forEach { row -> encodeSizedField(encoder, RowAdapter, row) }
        }

        override fun decode(decoder: NoritoDecoder): List<PrivacyCompiledProfileCatalogRowV1> {
            require(decoder.readLength(false) == ROW_COUNT.toLong()) {
                "compiled-profile catalog must declare exactly $ROW_COUNT rows"
            }
            val expected = PrivacyProtocolIdV1.values()
            return List(ROW_COUNT) { index ->
                val row = decodeBoundedSizedField(
                    decoder,
                    RowAdapter,
                    MAX_ROW_ENCODED_BYTES,
                    "catalog row $index",
                )
                require(row.protocolId == expected[index]) {
                    "compiled-profile catalog row $index is out of canonical protocol order"
                }
                row
            }
        }
    }

    private object RowAdapter : TypeAdapter<PrivacyCompiledProfileCatalogRowV1> {
        override fun encode(encoder: NoritoEncoder, value: PrivacyCompiledProfileCatalogRowV1) {
            encodeSizedField(encoder, ProtocolIdAdapter, value.protocolId)
            encodeSizedField(encoder, ProfileResultAdapter, value.compiledProfile)
        }

        override fun decode(decoder: NoritoDecoder): PrivacyCompiledProfileCatalogRowV1 {
            val protocol = decodeExactSizedField(
                decoder,
                ProtocolIdAdapter,
                4L,
                "catalog row protocol id",
            )
            val result = decodeBoundedSizedField(
                decoder,
                ProfileResultAdapter,
                MAX_PROFILE_RESULT_ENCODED_BYTES,
                "catalog row compiled profile",
            )
            return PrivacyCompiledProfileCatalogRowV1(protocol, result)
        }
    }

    private object ProfileResultAdapter : TypeAdapter<PrivacyCompiledProfileResultV1> {
        override fun encode(encoder: NoritoEncoder, value: PrivacyCompiledProfileResultV1) {
            when (value) {
                is PrivacyCompiledProfileResultV1.Available -> {
                    encoder.writeUInt(0L, 32)
                    encodeSizedField(encoder, ProfileAdapter, value.profile)
                }
                is PrivacyCompiledProfileResultV1.Unavailable -> {
                    encoder.writeUInt(1L, 32)
                    encodeSizedField(encoder, UnavailableReasonAdapter, value)
                }
            }
        }

        override fun decode(decoder: NoritoDecoder): PrivacyCompiledProfileResultV1 =
            when (val tag = decoder.readUInt(32)) {
                0L -> PrivacyCompiledProfileResultV1.Available(
                    decodeBoundedSizedField(
                        decoder,
                        ProfileAdapter,
                        MAX_PROFILE_ENCODED_BYTES,
                        "available compiled profile",
                    ),
                )
                1L -> decodeBoundedSizedField(
                    decoder,
                    UnavailableReasonAdapter,
                    32L,
                    "unavailable compiled-profile reason",
                )
                else -> throw IllegalArgumentException(
                    "unknown compiled-profile result discriminant: $tag",
                )
            }
    }

    private object ProfileAdapter : TypeAdapter<PrivacyCompiledProfileV1> {
        override fun encode(encoder: NoritoEncoder, value: PrivacyCompiledProfileV1) {
            encodeSizedField(encoder, ProtocolIdAdapter, value.protocolId)
            encodeSizedField(encoder, ProofSystemIdAdapter, value.proofSystemId)
            encodeSizedField(encoder, EngineIdAdapter, value.engineId)
            encodeSizedField(encoder, Fixed32Adapter, value.parameterId)
            encodeSizedField(encoder, Fixed32Adapter, value.parameterDigest)
            encodeSizedField(encoder, Fixed32Adapter, value.verifierDigest)
            encodeSizedField(encoder, Fixed32Adapter, value.statementSchemaDigest)
            encodeSizedField(encoder, Fixed32Adapter, value.engineManifestDigest)
            encodeSizedField(encoder, ProtocolLimitsAdapter, value.protocolLimits)
        }

        override fun decode(decoder: NoritoDecoder): PrivacyCompiledProfileV1 {
            val protocol = decodeExactSizedField(decoder, ProtocolIdAdapter, 4L, "profile protocol id")
            val proofSystem = decodeExactSizedField(
                decoder,
                ProofSystemIdAdapter,
                4L,
                "profile proof-system id",
            )
            val engine = decodeExactSizedField(decoder, EngineIdAdapter, 4L, "profile engine id")
            val parameterId = decodeFixed32Field(decoder, "profile parameter id")
            val parameterDigest = decodeFixed32Field(decoder, "profile parameter digest")
            val verifierDigest = decodeFixed32Field(decoder, "profile verifier digest")
            val statementSchemaDigest = decodeFixed32Field(
                decoder,
                "profile statement-schema digest",
            )
            val engineManifestDigest = decodeFixed32Field(
                decoder,
                "profile engine-manifest digest",
            )
            val limits = decodeBoundedSizedField(
                decoder,
                ProtocolLimitsAdapter,
                MAX_LIMITS_ENCODED_BYTES,
                "profile protocol limits",
            )
            return PrivacyCompiledProfileV1(
                protocol,
                proofSystem,
                engine,
                parameterId,
                parameterDigest,
                verifierDigest,
                statementSchemaDigest,
                engineManifestDigest,
                limits,
            )
        }
    }

    private object ProtocolIdAdapter : TypeAdapter<PrivacyProtocolIdV1> {
        override fun encode(encoder: NoritoEncoder, value: PrivacyProtocolIdV1) {
            encoder.writeUInt(value.ordinal.toLong(), 32)
        }

        override fun decode(decoder: NoritoDecoder): PrivacyProtocolIdV1 =
            enumValue(decoder.readUInt(32), PrivacyProtocolIdV1.values(), "protocol")
    }

    private object ProofSystemIdAdapter : TypeAdapter<PrivacyProofSystemIdV1> {
        override fun encode(encoder: NoritoEncoder, value: PrivacyProofSystemIdV1) {
            encoder.writeUInt(value.ordinal.toLong(), 32)
        }

        override fun decode(decoder: NoritoDecoder): PrivacyProofSystemIdV1 =
            enumValue(decoder.readUInt(32), PrivacyProofSystemIdV1.values(), "proof-system")
    }

    private object EngineIdAdapter : TypeAdapter<PrivacyEngineIdV1> {
        override fun encode(encoder: NoritoEncoder, value: PrivacyEngineIdV1) {
            encoder.writeUInt(value.ordinal.toLong(), 32)
        }

        override fun decode(decoder: NoritoDecoder): PrivacyEngineIdV1 =
            enumValue(decoder.readUInt(32), PrivacyEngineIdV1.values(), "engine")
    }

    private object Fixed32Adapter : TypeAdapter<PrivacyFixed32V1> {
        override fun encode(encoder: NoritoEncoder, value: PrivacyFixed32V1) {
            encoder.writeLength(32L, true)
            encoder.writeBytes(value.bytes())
        }

        override fun decode(decoder: NoritoDecoder): PrivacyFixed32V1 {
            require(decoder.readLength(true) == 32L) {
                "compiled-profile binding wrapper must declare exactly 32 bytes"
            }
            return PrivacyFixed32V1(decoder.readBytes(32))
        }
    }

    private object ProtocolLimitsAdapter : TypeAdapter<PrivacyProtocolLimitsV1> {
        override fun encode(encoder: NoritoEncoder, value: PrivacyProtocolLimitsV1) {
            val protocol = value.protocolId
            encoder.writeUInt(protocol.ordinal.toLong(), 32)
            if (privacyProtocolLimitRulesV1(protocol).isNotEmpty()) {
                encodeSizedField(encoder, LimitPayloadAdapter(protocol), value)
            }
        }

        override fun decode(decoder: NoritoDecoder): PrivacyProtocolLimitsV1 {
            val protocol = enumValue(
                decoder.readUInt(32),
                PrivacyProtocolIdV1.values(),
                "protocol-limit",
            )
            return if (privacyProtocolLimitRulesV1(protocol).isEmpty()) {
                PrivacyProtocolLimitsV1(protocol, null)
            } else {
                decodeBoundedSizedField(
                    decoder,
                    LimitPayloadAdapter(protocol),
                    64L,
                    "protocol-limit payload",
                )
            }
        }
    }

    private class LimitPayloadAdapter(
        private val protocol: PrivacyProtocolIdV1,
    ) : TypeAdapter<PrivacyProtocolLimitsV1> {
        override fun encode(encoder: NoritoEncoder, value: PrivacyProtocolLimitsV1) {
            require(value.protocolId == protocol) { "protocol-limit payload tag mismatch" }
            val fields = requireNotNull(value.values)
            for (rule in privacyProtocolLimitRulesV1(protocol)) {
                encodeSizedField(encoder, UINT32_ADAPTER, fields.getValue(rule.name).toLong())
            }
        }

        override fun decode(decoder: NoritoDecoder): PrivacyProtocolLimitsV1 {
            val values = linkedMapOf<String, Int>()
            for (rule in privacyProtocolLimitRulesV1(protocol)) {
                val value = decodeExactSizedField(
                    decoder,
                    UINT32_ADAPTER,
                    4L,
                    "protocol limit ${rule.name}",
                )
                require(value <= Int.MAX_VALUE.toLong()) {
                    "protocol limit ${rule.name} exceeds the supported JVM integer range"
                }
                values[rule.name] = value.toInt()
            }
            return PrivacyProtocolLimitsV1(protocol, values)
        }
    }

    private object UnavailableReasonAdapter :
        TypeAdapter<PrivacyCompiledProfileResultV1.Unavailable> {
        override fun encode(
            encoder: NoritoEncoder,
            value: PrivacyCompiledProfileResultV1.Unavailable,
        ) {
            when (value.reason) {
                PrivacyCompiledProfileUnavailableReasonV1.ENGINE_UNAVAILABLE ->
                    encoder.writeUInt(0L, 32)
                PrivacyCompiledProfileUnavailableReasonV1.PROFILE_INITIALIZATION_FAILED ->
                    encoder.writeUInt(1L, 32)
                PrivacyCompiledProfileUnavailableReasonV1.STATEMENT_SCHEMA_INVALID -> {
                    encoder.writeUInt(2L, 32)
                    encodeSizedField(
                        encoder,
                        StatementSchemaErrorAdapter,
                        requireNotNull(value.statementSchemaError),
                    )
                }
            }
        }

        override fun decode(decoder: NoritoDecoder): PrivacyCompiledProfileResultV1.Unavailable =
            when (val tag = decoder.readUInt(32)) {
                0L -> PrivacyCompiledProfileResultV1.Unavailable(
                    PrivacyCompiledProfileUnavailableReasonV1.ENGINE_UNAVAILABLE,
                    null,
                )
                1L -> PrivacyCompiledProfileResultV1.Unavailable(
                    PrivacyCompiledProfileUnavailableReasonV1.PROFILE_INITIALIZATION_FAILED,
                    null,
                )
                2L -> PrivacyCompiledProfileResultV1.Unavailable(
                    PrivacyCompiledProfileUnavailableReasonV1.STATEMENT_SCHEMA_INVALID,
                    decodeExactSizedField(
                        decoder,
                        StatementSchemaErrorAdapter,
                        4L,
                        "statement-schema error",
                    ),
                )
                else -> throw IllegalArgumentException(
                    "unknown compiled-profile unavailable-reason discriminant: $tag",
                )
            }
    }

    private object StatementSchemaErrorAdapter : TypeAdapter<PrivacyCompiledStatementSchemaErrorV1> {
        override fun encode(encoder: NoritoEncoder, value: PrivacyCompiledStatementSchemaErrorV1) {
            encoder.writeUInt(value.ordinal.toLong(), 32)
        }

        override fun decode(decoder: NoritoDecoder): PrivacyCompiledStatementSchemaErrorV1 =
            enumValue(
                decoder.readUInt(32),
                PrivacyCompiledStatementSchemaErrorV1.values(),
                "statement-schema error",
            )
    }

    private fun decodeFixed32Field(decoder: NoritoDecoder, fieldName: String): PrivacyFixed32V1 =
        decodeExactSizedField(
            decoder,
            Fixed32Adapter,
            FIXED32_WRAPPER_ENCODED_BYTES,
            fieldName,
        )

    private fun <T> enumValue(tag: Long, values: Array<T>, kind: String): T {
        require(tag >= 0L && tag < values.size.toLong()) { "unknown $kind discriminant: $tag" }
        return values[tag.toInt()]
    }

    private fun <T> encodeSizedField(encoder: NoritoEncoder, adapter: TypeAdapter<T>, value: T) {
        val child = encoder.childEncoder()
        adapter.encode(child, value)
        val payload = child.toByteArray()
        encoder.writeLength(payload.size.toLong(), true)
        encoder.writeBytes(payload)
    }

    private fun <T> decodeExactSizedField(
        decoder: NoritoDecoder,
        adapter: TypeAdapter<T>,
        expectedEncodedLength: Long,
        fieldName: String,
    ): T {
        val actual = decoder.readLength(true)
        require(actual == expectedEncodedLength) {
            "$fieldName must contain exactly $expectedEncodedLength encoded bytes"
        }
        return decodeChild(decoder, adapter, actual.toInt(), fieldName)
    }

    private fun <T> decodeBoundedSizedField(
        decoder: NoritoDecoder,
        adapter: TypeAdapter<T>,
        maximumEncodedLength: Long,
        fieldName: String,
    ): T {
        val length = decoder.readLength(true)
        require(length in 0L..maximumEncodedLength) {
            "$fieldName exceeds its encoded byte limit"
        }
        return decodeChild(decoder, adapter, length.toInt(), fieldName)
    }

    private fun <T> decodeChild(
        decoder: NoritoDecoder,
        adapter: TypeAdapter<T>,
        length: Int,
        fieldName: String,
    ): T {
        val child = NoritoDecoder(decoder.readBytes(length), decoder.flags, decoder.flagsHint)
        val value = adapter.decode(child)
        require(child.remaining() == 0) { "$fieldName contains trailing or unknown data" }
        return value
    }

    private fun decodeHex(value: String): ByteArray {
        require(value.length % 2 == 0) { "hex must contain complete bytes" }
        return ByteArray(value.length / 2) { index ->
            val offset = index * 2
            value.substring(offset, offset + 2).toInt(16).toByte()
        }
    }
}
