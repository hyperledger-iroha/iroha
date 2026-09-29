package org.hyperledger.iroha.sdk.consensus

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address
import org.hyperledger.iroha.sdk.core.model.instructions.TransferWirePayloadEncoder
import org.hyperledger.iroha.sdk.core.util.HashLiteral
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash

/**
 * Cross-language parity for the exact artifact emitted by the native replay owner.
 * This test consumer does not authenticate BLS certificates or execute a block.
 * Kagami must independently replay every carrier, context and input/output proof
 * against its retained launch plan before producing either fixture document.
 */
object NativeExecutionEvidenceFixtures {
    const val SCHEMA = "iroha_kagami::scaling_evidence::ExportEnvelopeV1"
    const val MAX_BYTES = 32 * 1024 * 1024
    private const val MAX_ITEMS = 4096
    private val envelopeFields = setOf("version", "artifact_schema", "artifact_hash", "canonical_artifact_hex", "requests")
    private val rowFields = setOf("logical_id", "phase", "sequence", "authority", "entrypoint_hash", "carrier_height", "carrier_hash", "lane_source", "leaf_index", "lane_id", "dataspace_id")
    private val sourceFields = setOf("incarnation", "instance", "height", "block_hash", "result", "batch_index", "anchor_height", "anchor_hash")
    private val hexPattern = Regex("^[0-9a-f]+$")

    @JvmStatic fun load(lanes: Int): ByteArray {
        require(lanes == 1 || lanes == 4)
        var root: Path? = Paths.get("").toAbsolutePath()
        while (root != null) {
            val file = root.resolve("fixtures/sumeragi/native_execution_evidence_${lanes}_lanes_v1.json")
            if (Files.isRegularFile(file)) {
                require(Files.size(file) in 1..MAX_BYTES.toLong())
                return Files.readAllBytes(file)
            }
            root = root.parent
        }
        error("Run the native Kagami fixture capture; a missing fixture is not a skipped test")
    }

    @JvmStatic fun inspect(document: String): List<Map<String, Any?>> = inspect(document.toByteArray(StandardCharsets.UTF_8))

    /** Return exact projected rows only after checking every byte binding. */
    @JvmStatic fun inspect(document: ByteArray): List<Map<String, Any?>> {
        require(document.size in 1..MAX_BYTES)
        val p = SumeragiJsonPrimitives
        val root = p.exactObject(p.parseObject(p.decodeUtf8(document, "native evidence"), "native evidence"), envelopeFields, "native evidence")
        require(p.u16(root["version"], "version") == 1 && root["artifact_schema"] == SCHEMA)
        val artifact = hex(root["canonical_artifact_hex"], MAX_BYTES)
        val expectedHash = p.hash(root["artifact_hash"], "artifact_hash")
        require(HashLiteral.decode(expectedHash).contentEquals(IrohaHash.prehash(artifact))) { "artifact hash mismatch" }
        val rows = p.array(root["requests"], "requests", MAX_ITEMS).map { p.exactObject(it, rowFields, "request") }
        require(rows.isNotEmpty())
        val r = Reader(frame(artifact, SCHEMA))
        require(r.number(2) == BigInteger.ONE)
        val heights = Reader(r.field()).sequence { bytes ->
            val h = Reader(bytes)
            val height = h.number(8)
            require(height.signum() > 0)
            val carrier = Reader(h.field()).byteVector()
            require(carrier.isNotEmpty() && carrier[0].toInt() == 1)
            frame(carrier.copyOfRange(1, carrier.size), "iroha_data_model::block::model::SignedBlock")
            val evidence = Reader(frame(Reader(h.field()).byteVector(), "iroha_kagami::scaling_evidence::LaneMergeEvidenceV1"))
            val state = Reader(evidence.field())
            require(state.number(8) == height)
            val carrierHash = state.fixed(32)
            require(carrierHash[31].toInt() and 1 == 1)
            // The Rust replay authenticates complete lanes and ordered ordinary writes
            // against their native result roots, then verifies every original lane frame.
            state.field()
            Reader(state.field()).sequence { bytes ->
                val write = Reader(bytes)
                Reader(write.field()).byteVector()
                Reader(write.field()).byteVector()
                write.finish()
            }
            require(Reader(state.field()).sequence { it }.isEmpty()) {
                "these native workload fixtures have no Parliament casting bindings"
            }
            state.finish()
            Reader(evidence.field()).sequence { bytes ->
                val original = Reader(bytes)
                val lane = Reader(original.field())
                require(lane.number(4).signum() > 0)
                lane.finish()
                val frame = Reader(Reader(original.field()).byteVector())
                frame.field(); frame.field(); frame.finish()
                original.finish()
            }
            evidence.finish()
            val queries = Reader(h.field()).sequence { query ->
                frame(Reader(query).byteVector(), "iroha_data_model::query::model::CommittedTransaction")
            }
            h.finish()
            Triple(height, carrierHash, queries.size)
        }
        require(heights.isNotEmpty())
        heights.zipWithNext().forEach { (a, b) -> require(b.first == a.first + BigInteger.ONE) }
        val encodedRows = Reader(r.field()).sequence { it }
        r.finish()
        require(encodedRows.size == rows.size)
        val ids = mutableSetOf<String>()
        val applicationSlots = mutableSetOf<Pair<BigInteger, BigInteger>>()
        val sequences = mutableMapOf("warmup" to BigInteger.ZERO, "measurement" to BigInteger.ZERO)
        var measurement = false
        rows.zip(encodedRows).forEach { (row, encoded) ->
            val phase = row["phase"] as? String ?: error("phase must be a string")
            require(phase in sequences)
            if (phase == "measurement") measurement = true else require(!measurement)
            val sequence = p.positiveU64(row["sequence"], "sequence")
            require(sequence == sequences.getValue(phase) + BigInteger.ONE)
            sequences[phase] = sequence
            val logical = row["logical_id"] as? String ?: error("logical ID must be a string")
            require(logical.length == 64 && hexPattern.matches(logical) && ids.add(logical))
            val height = p.positiveU64(row["carrier_height"], "carrier_height")
            val leaf = p.u32(row["leaf_index"], "leaf_index")
            require(applicationSlots.add(height to leaf))
            val carrier = heights.singleOrNull { it.first == height } ?: error("row has no carrier")
            require(leaf < carrier.third.toBigInteger())
            require(hexHash(row["carrier_hash"]).contentEquals(carrier.second))
            require(encodeRow(row).contentEquals(encoded)) { "JSON request differs from canonical artifact row" }
        }
        require(sequences.values.all { it.signum() > 0 })
        return rows
    }

    private fun encodeRow(row: Map<String, Any?>): ByteArray {
        val p = SumeragiJsonPrimitives
        fun number(name: String, size: Int): ByteArray = integer(if (size == 4) p.u32(row[name], name) else p.u64(row[name], name), size)
        val logical = (row.getValue("logical_id") as String).toByteArray(StandardCharsets.UTF_8)
        val authority = requireCanonicalI105Address(row["authority"] as? String ?: error("authority must be a string"), "authority")
        val request = record(length(logical.size) + logical,
            integer(if (row["phase"] == "warmup") BigInteger.ZERO else BigInteger.ONE, 4),
            TransferWirePayloadEncoder.encodeAccountIdPayload(authority),
            hexHash(row["entrypoint_hash"]), number("carrier_height", 8), hexHash(row["carrier_hash"]),
            encodeSource(row["lane_source"], p.u32(row["lane_id"], "lane_id"), p.positiveU64(row["carrier_height"], "carrier_height")),
            number("leaf_index", 4), record(number("lane_id", 4)), record(number("dataspace_id", 8)))
        return record(number("sequence", 8), request)
    }

    private fun encodeSource(value: Any?, lane: BigInteger, carrierHeight: BigInteger): ByteArray {
        if (value == null) { require(lane == BigInteger.ZERO); return byteArrayOf(0) }
        require(lane.signum() > 0)
        val p = SumeragiJsonPrimitives
        val source = p.exactObject(value, sourceFields, "lane_source")
        fun raw(name: String) = hex(source[name], 32).also { require(it.size == 32) }
        val height = p.positiveU64(source["height"], "height")
        val anchor = p.positiveU64(source["anchor_height"], "anchor_height")
        require(anchor < carrierHeight)
        val body = record(raw("incarnation"), raw("instance"), integer(height, 8),
            raw("block_hash"), raw("result"), integer(p.u32(source["batch_index"], "batch_index"), 4),
            integer(anchor, 8), hexHash(source["anchor_hash"]))
        return byteArrayOf(1) + record(body)
    }

    internal fun frame(wire: ByteArray, schema: String): ByteArray {
        require(wire.size in NoritoHeader.HEADER_LENGTH..MAX_BYTES)
        require(wire[22].toInt() == 0 && wire[39].toInt() == NoritoHeader.COMPACT_LEN)
        val decoded = NoritoHeader.decode(wire, SchemaHash.hash16(schema))
        val canonical = NoritoHeader(SchemaHash.hash16(schema), decoded.payload.size, CRC64.compute(decoded.payload), NoritoHeader.COMPACT_LEN, 0).encode() + decoded.payload
        require(canonical.contentEquals(wire)) { "noncanonical frame" }
        return decoded.payload
    }

    private fun hexHash(value: Any?): ByteArray = hex(value, 32).also {
        require(it.size == 32 && it[31].toInt() and 1 == 1) { "expected canonical lowercase Iroha hash" }
    }
    private fun hex(value: Any?, maximum: Int): ByteArray {
        require(value is String && value.length in 2..maximum * 2 && value.length % 2 == 0 && hexPattern.matches(value))
        return ByteArray(value.length / 2) { i -> value.substring(i * 2, i * 2 + 2).toInt(16).toByte() }
    }
    internal fun integer(value: BigInteger, size: Int): ByteArray {
        require(value.signum() >= 0 && value.bitLength() <= size * 8)
        return ByteArray(size) { value.shiftRight(it * 8).toByte() }
    }
    internal fun length(value: Int): ByteArray {
        require(value >= 0)
        var n = value
        val out = ByteArrayOutputStream()
        do { val b = n and 127; n = n ushr 7; out.write(b or if (n == 0) 0 else 128) } while (n != 0)
        return out.toByteArray()
    }
    internal fun record(vararg fields: ByteArray): ByteArray {
        val out = ByteArrayOutputStream()
        fields.forEach { out.write(length(it.size)); out.write(it) }
        return out.toByteArray()
    }
    internal class Reader(private val bytes: ByteArray) {
        private var cursor = 0
        private fun remaining() = bytes.size - cursor
        fun raw(count: Int): ByteArray {
            require(count >= 0 && count <= remaining())
            return bytes.copyOfRange(cursor, cursor + count).also { cursor += count }
        }
        fun finish() = require(remaining() == 0) { "trailing evidence bytes" }
        fun field(): ByteArray {
            var value = 0; var shift = 0; var count = 0
            while (true) {
                require(shift <= 28)
                val b = raw(1)[0].toInt() and 255
                require(shift < 28 || b <= 7)
                value = value or ((b and 127) shl shift); count++
                if (b and 128 == 0) break
                shift += 7
            }
            require(value <= MAX_BYTES && length(value).size == count)
            return raw(value)
        }
        fun fixed(size: Int) = field().also { require(it.size == size) }
        fun number(size: Int) = BigInteger(1, fixed(size).reversedArray())
        private fun count(maximum: Int): Int {
            val n = BigInteger(1, raw(8).reversedArray())
            require(n <= maximum.toBigInteger() && n <= remaining().toBigInteger())
            return n.toInt()
        }
        fun byteVector(): ByteArray = raw(count(MAX_BYTES)).also { finish() }
        fun <T> sequence(decode: (ByteArray) -> T): List<T> {
            val count = count(MAX_ITEMS)
            val values = List(count) { decode(field()) }
            finish()
            return values
        }
    }
}
