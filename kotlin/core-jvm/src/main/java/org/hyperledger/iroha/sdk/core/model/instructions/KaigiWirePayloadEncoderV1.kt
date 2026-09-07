package org.hyperledger.iroha.sdk.core.model.instructions

import java.util.Base64
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.core.model.JsonValue
import org.hyperledger.iroha.sdk.core.model.WirePayload
import org.hyperledger.iroha.sdk.core.util.HashLiteral
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter

/** Typed Kaigi instructions produce the actual immutable wire payload captured by InstructionBox. */
interface KaigiWireInstructionV1 : InstructionTemplate, WirePayload {
    override val wireName: String get() = KaigiWirePayloadEncoderV1.wireName(arguments.getValue("action"))
    override val payloadBytes: ByteArray get() = KaigiWirePayloadEncoderV1.encode(arguments)
}

/** Final V1 Kaigi Norito owner for Kotlin and Java callers; proof authorization remains with Core. */
object KaigiWirePayloadEncoderV1 {
    private const val PREFIX = "iroha.instruction.v1::kaigi::"
    private const val SCHEMA_PREFIX = "iroha_data_model::isi::kaigi::"
    private const val MAX_BYTES = 64 * 1024 * 1024
    private val ACTIONS = setOf("CreateKaigi", "JoinKaigi", "LeaveKaigi", "EndKaigi", "RecordKaigiUsage")
    private val NONE = byteArrayOf(0)
    private object Raw : TypeAdapter<ByteArray> {
        override fun encode(encoder: NoritoEncoder, value: ByteArray) { encoder.writeBytes(value) }
        override fun decode(decoder: NoritoDecoder): ByteArray = decoder.readBytes(decoder.remaining())
    }

    internal fun wireName(action: String): String {
        require(action in ACTIONS) { "unknown Kaigi V1 instruction" }
        return PREFIX + action
    }

    internal fun encode(arguments: Map<String, String>): ByteArray {
        val action = arguments.getValue("action")
        wireName(action)
        val a = parse(arguments).arguments
        val fields = ArrayList<ByteArray>()
        if (action == "CreateKaigi") {
            val call = struct(
                callId(a), account(a.getValue("host")),
                option(a["title"]?.let(::string)), option(a["description"]?.let(::string)),
                option(a["max_participants"]?.let { uint(KaigiInstructionUtils.parsePositiveInt(it, "max_participants").toLong(), 32) }),
                u64(a.getValue("gas_rate_per_minute")), metadata(a),
                option(a["scheduled_start_ms"]?.let(::u64)), option(a["billing_account"]?.let(::account)),
                uint(if (a.getValue("privacy.mode") == "Transparent") 0 else 1, 32),
                uint(if (a.getValue("room_policy.policy") == "Public") 0 else 1, 32),
                option(relayManifest(a)),
            )
            fields.add(call)
        } else {
            fields.add(callId(a))
            when (action) {
                "JoinKaigi", "LeaveKaigi" -> fields.add(account(a.getValue("participant")))
                "EndKaigi" -> fields.add(option(a["ended_at_ms"]?.let(::u64)))
                "RecordKaigiUsage" -> {
                    fields.add(u64(a.getValue("duration_ms")))
                    fields.add(u64(a.getValue("billed_gas")))
                }
            }
        }
        if (action == "RecordKaigiUsage") {
            fields.add(option(a["usage_commitment"]?.let { KaigiAuthorizationScalarV1.fromArgument(it).toLeBytes() }))
        } else {
            fields.add(option(a["commitment.commitment"]?.let { struct(KaigiAuthorizationScalarV1.fromArgument(it).toLeBytes()) }))
            fields.add(option(a["nullifier.digest"]?.let { struct(KaigiAuthorizationScalarV1.fromArgument(it).toLeBytes()) }))
            fields.add(option(a["roster_root"]?.let { HashLiteral.decode(KaigiInstructionUtils.canonicalRosterRoot(it)) }))
        }
        fields.add(option(a["proof"]?.let { rawVector(proof(it)) }))
        return NoritoCodec.encode(struct(*fields.toTypedArray()), SCHEMA_PREFIX + action, Raw).also {
            require(it.size <= MAX_BYTES) { "Kaigi archive exceeds the V1 byte limit" }
        }
    }

    /** Validate recognized Kaigi frames at the transaction boundary, including caller-supplied WirePayloads. */
    @JvmStatic
    fun requireCanonicalIfKnown(wireName: String, frame: ByteArray) {
        if (!wireName.startsWith(PREFIX) || wireName.removePrefix(PREFIX) !in ACTIONS) return
        val decoded = decode(wireName, frame)
        require(decoded.payloadBytes.contentEquals(frame)) { "Kaigi frame is not the final canonical V1 encoding" }
    }

    /** Decode a complete canonical frame into the same typed instruction constructors used for creation. */
    @JvmStatic
    fun decode(wireName: String, frame: ByteArray): KaigiWireInstructionV1 {
        val action = wireName.removePrefix(PREFIX)
        require(wireName == wireName(action)) { "unknown Kaigi V1 wire identifier" }
        require(frame.size in 40..MAX_BYTES && (frame[39].toInt() and 255) == NoritoHeader.COMPACT_LEN && frame[22] == 0.toByte()) {
            "Kaigi V1 requires the exact uncompressed compact-length Norito frame"
        }
        val reader = Reader(NoritoCodec.decode(frame, Raw, SCHEMA_PREFIX + action))
        val a = linkedMapOf("action" to action)
        if (action == "CreateKaigi") {
            val call = Reader(reader.field())
            decodeCall(call.field(), a)
            a["host"] = readAccount(call.field())
            call.option()?.let { a["title"] = readString(it) }
            call.option()?.let { a["description"] = readString(it) }
            call.option()?.let { a["max_participants"] = java.lang.Long.toUnsignedString(readUInt(it, 32)) }
            a["gas_rate_per_minute"] = readU64(call.field())
            decodeMetadata(call.field(), a)
            call.option()?.let { a["scheduled_start_ms"] = readU64(it) }
            call.option()?.let { a["billing_account"] = readAccount(it) }
            a["privacy.mode"] = when (readUInt(call.field(), 32)) { 0L -> "Transparent"; 1L -> "ZkRosterV1"; else -> throw IllegalArgumentException("unknown Kaigi privacy mode") }
            a["room_policy.policy"] = when (readUInt(call.field(), 32)) { 0L -> "Public"; 1L -> "Authenticated"; else -> throw IllegalArgumentException("unknown Kaigi room policy") }
            call.option()?.let { decodeRelayManifest(it, a) }
            call.end()
        } else {
            decodeCall(reader.field(), a)
            when (action) {
                "JoinKaigi", "LeaveKaigi" -> a["participant"] = readAccount(reader.field())
                "EndKaigi" -> reader.option()?.let { a["ended_at_ms"] = readU64(it) }
                "RecordKaigiUsage" -> {
                    a["duration_ms"] = readU64(reader.field())
                    a["billed_gas"] = readU64(reader.field())
                }
            }
        }
        if (action == "RecordKaigiUsage") {
            reader.option()?.let { a["usage_commitment"] = KaigiAuthorizationScalarV1.fromLeBytes(it).toHex() }
        } else {
            for (key in listOf("commitment.commitment", "nullifier.digest")) {
                reader.option()?.let { payload ->
                    val scalar = Reader(payload)
                    a[key] = KaigiAuthorizationScalarV1.fromLeBytes(scalar.field()).toHex()
                    scalar.end() // The retired alias/timestamp slots have no final V1 representation.
                }
            }
            reader.option()?.let {
                require(it.size == 32 && (it.last().toInt() and 1) == 1) { "invalid Kaigi roster Hash" }
                a["roster_root"] = HashLiteral.canonicalize(it)
            }
        }
        reader.option()?.let { a["proof"] = Base64.getEncoder().encodeToString(readRawVector(it)) }
        reader.end()
        val result = parse(a)
        require(result.payloadBytes.contentEquals(frame)) { "noncanonical Kaigi V1 frame" }
        return result
    }

    private fun parse(a: Map<String, String>): KaigiWireInstructionV1 =
        when (a.getValue("action")) {
            "CreateKaigi" -> CreateKaigiInstruction.fromArguments(a)
            "JoinKaigi" -> JoinKaigiInstruction.fromArguments(a)
            "LeaveKaigi" -> LeaveKaigiInstruction.fromArguments(a)
            "EndKaigi" -> EndKaigiInstruction.fromArguments(a)
            "RecordKaigiUsage" -> RecordKaigiUsageInstruction.fromArguments(a)
            else -> throw IllegalArgumentException("unknown Kaigi V1 instruction")
        }

    private fun encoder(): NoritoEncoder = NoritoEncoder(NoritoHeader.COMPACT_LEN)
    private fun field(bytes: ByteArray): ByteArray = encoder().also { it.writeLength(bytes.size.toLong(), true); it.writeBytes(bytes) }.toByteArray()
    private fun struct(vararg fields: ByteArray): ByteArray = encoder().also { e -> fields.forEach { e.writeBytes(field(it)) } }.toByteArray()
    private fun uint(value: Long, bits: Int): ByteArray = encoder().also { it.writeUInt(value, bits) }.toByteArray()
    private fun u64(value: String): ByteArray = uint(KaigiInstructionUtils.parseUnsignedLong(value, "u64"), 64)
    private fun option(value: ByteArray?): ByteArray = if (value == null) NONE.copyOf() else byteArrayOf(1) + field(value)
    private fun string(value: String): ByteArray {
        val bytes = value.toByteArray(Charsets.UTF_8)
        require(String(bytes, Charsets.UTF_8) == value) { "Kaigi string must contain valid Unicode" }
        return field(bytes)
    }
    private fun name(value: String): ByteArray {
        // TODO: use the same pinned Rust NFC/UTS-46 owner before accepting Unicode identity names.
        require(value.isNotEmpty() && value.length <= 255 && value.all { it.code in 33..126 && it !in "@#$" }) {
            "Kaigi identity names must use canonical nonempty ASCII Name bytes"
        }
        return string(value)
    }
    private fun domain(value: String): ByteArray {
        val parts = value.split('.')
        require(parts.size == 2 && parts.all {
            it.matches(Regex("[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?")) && !it.startsWith("xn--") && (it.length < 4 || it.substring(2, 4) != "--")
        }) { "Kaigi domain must use canonical ASCII domain.dataspace labels" }
        return struct(name(parts[0]), name(parts[1]))
    }
    private fun callId(a: Map<String, String>): ByteArray = struct(domain(a.getValue("call.domain_id")), name(a.getValue("call.call_name")))
    private fun account(value: String): ByteArray {
        val prefix = requireNotNull(AccountAddress.detectI105Discriminant(value)) { "Kaigi account must use an exact sentinel-prefixed I105 identifier" }
        val bytes = TransferWirePayloadEncoder.encodeAccountIdPayload(value)
        require(TransferWirePayloadEncoder.decodeAccountIdPayload(bytes, prefix) == value) { "noncanonical Kaigi AccountId" }
        return bytes
    }
    private fun rawVector(bytes: ByteArray): ByteArray = uint(bytes.size.toLong(), 64) + bytes
    private fun vector(values: List<ByteArray>): ByteArray = uint(values.size.toLong(), 64) + encoder().also { e -> values.forEach { e.writeBytes(field(it)) } }.toByteArray()
    private fun proof(value: String): ByteArray {
        require(value.length <= (MAX_BYTES / 3 + 1) * 4) { "Kaigi proof exceeds the V1 byte limit" }
        KaigiInstructionUtils.requireBase64(value, "proof")
        return Base64.getDecoder().decode(value).also { require(it.size <= MAX_BYTES) }
    }
    private fun metadata(a: Map<String, String>): ByteArray = vector(a.entries.filter { it.key.startsWith("metadata.") }.sortedBy { it.key }.map {
        val json = JsonValue.parse(it.value).canonicalJson
        require(json.toByteArray(Charsets.UTF_8).size <= 1_048_576) { "Kaigi metadata JSON exceeds the V1 limit" }
        struct(name(it.key.removePrefix("metadata.")), struct(string(json)))
    })
    private fun relayManifest(a: Map<String, String>): ByteArray? {
        val manifest = KaigiInstructionUtils.parseRelayManifest(a, "relay_manifest") ?: return null
        return struct(vector(manifest.hops.map {
            struct(account(requireNotNull(it.relayId)), rawVector(Base64.getDecoder().decode(it.hpkePublicKey)), uint(requireNotNull(it.weight).toLong(), 8))
        }), uint(requireNotNull(manifest.expiryMs), 64))
    }

    private class Reader(bytes: ByteArray) {
        val decoder = NoritoDecoder(bytes, NoritoHeader.COMPACT_LEN)
        fun field(): ByteArray {
            val length = decoder.readLength(true)
            require(length in 0..decoder.remaining().toLong()) { "invalid Kaigi field length" }
            return decoder.readBytes(length.toInt())
        }
        fun option(): ByteArray? {
            val child = Reader(field())
            val tag = child.decoder.readUInt(8)
                val result = when (tag) { 0L -> null; 1L -> child.field(); else -> throw IllegalArgumentException("invalid Kaigi option tag") }
            child.end()
            return result
        }
        fun end() { require(decoder.remaining() == 0) { "trailing bytes in Kaigi V1 struct" } }
    }
    private fun readString(bytes: ByteArray): String {
        val reader = Reader(bytes)
        val raw = reader.field(); reader.end()
        return String(raw, Charsets.UTF_8).also { require(it.toByteArray(Charsets.UTF_8).contentEquals(raw)) { "invalid Kaigi UTF-8" } }
    }
    private fun readUInt(bytes: ByteArray, bits: Int): Long {
        require(bytes.size == bits / 8) { "invalid Kaigi integer width" }
        return NoritoDecoder(bytes, NoritoHeader.COMPACT_LEN).readUInt(bits)
    }
    private fun readU64(bytes: ByteArray): String = java.lang.Long.toUnsignedString(readUInt(bytes, 64))
    private fun readAccount(bytes: ByteArray): String = TransferWirePayloadEncoder.decodeAccountIdPayload(bytes, AccountAddress.DEFAULT_I105_DISCRIMINANT)
    private fun decodeCall(bytes: ByteArray, a: MutableMap<String, String>) {
        val call = Reader(bytes); val domain = Reader(call.field())
        a["call.domain_id"] = readString(domain.field()) + "." + readString(domain.field()); domain.end()
        a["call.call_name"] = readString(call.field()); call.end()
    }
    private fun sequence(bytes: ByteArray, max: Int): List<ByteArray> {
        val reader = Reader(bytes); val count = reader.decoder.readUInt(64)
        require(count in 0..max.toLong() && count <= reader.decoder.remaining()) { "Kaigi sequence exceeds its field bounds" }
        return List(count.toInt()) { reader.field() }.also { reader.end() }
    }
    private fun readRawVector(bytes: ByteArray): ByteArray {
        val reader = Reader(bytes); val count = reader.decoder.readUInt(64)
        require(count == reader.decoder.remaining().toLong()) { "invalid Kaigi byte vector length" }
        return reader.decoder.readBytes(count.toInt())
    }
    private fun decodeMetadata(bytes: ByteArray, a: MutableMap<String, String>) {
        sequence(bytes, 65_536).forEach {
            val pair = Reader(it); val key = "metadata." + readString(pair.field()); val json = Reader(pair.field())
            require(key !in a) { "duplicate Kaigi metadata key" }
            a[key] = readString(json.field()); json.end(); pair.end()
        }
    }
    private fun decodeRelayManifest(bytes: ByteArray, a: MutableMap<String, String>) {
        val manifest = Reader(bytes)
        sequence(manifest.field(), 8).forEachIndexed { index, bytes ->
            val hop = Reader(bytes); val prefix = "relay_manifest.hop.$index."
            a[prefix + "relay_id"] = readAccount(hop.field())
            a[prefix + "hpke_public_key"] = Base64.getEncoder().encodeToString(readRawVector(hop.field()))
            a[prefix + "weight"] = readUInt(hop.field(), 8).toString(); hop.end()
        }
        a["relay_manifest.expiry_ms"] = readU64(manifest.field()); manifest.end()
    }
}
