package org.hyperledger.iroha.sdk.consensus

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import org.hyperledger.iroha.sdk.address.*
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.core.util.HashLiteral
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash

/** Sole native status Norito frame; bare payloads, alternate layouts and old status are rejected. */
object SumeragiStatusWire {
    private const val SCHEMA = "iroha_data_model::sumeragi::SumeragiStatus"
    private const val LIMIT = 1_048_576
    @JvmStatic fun encode(value: SumeragiStatus): ByteArray {
        val payload = record(integer(value.protocolVersion.toBigInteger(), 2),
            HashLiteral.decode(value.configFingerprint),
            option(value.beaconHorizon?.let(::encodeHorizon)),
            hex(value.instance),
            integer(value.height, 8),
            integer(value.view, 8),
            integer(value.stage.toBigInteger(), 1),
            option(value.leader?.let(::encodeKey)),
            option(value.proxyTail?.let(::encodeKey)),
            option(value.highQcView?.let { integer(it, 8) }),
            integer(value.level, 4),
            integer(value.startLevel, 4),
            integer(value.tRetxMs, 8),
            integer(value.committedHeight, 8),
            integer(value.appliedHeight, 8),
            boolean(value.awaiting),
            option(value.signer?.let(::encodeKey)),
            boolean(value.unanchored),
            boolean(value.abstaining),
            option(value.halted?.let(::encodeHalt)),
            encodeFootprint(value.footprint))
        require(payload.size <= LIMIT - NoritoHeader.HEADER_LENGTH)
        return NoritoHeader(SchemaHash.hash16(SCHEMA), payload.size, CRC64.compute(payload), NoritoHeader.COMPACT_LEN, NoritoHeader.COMPRESSION_NONE).encode() + payload
    }
    @JvmStatic fun decodeCanonical(wire: ByteArray): SumeragiStatus {
        require(wire.size in NoritoHeader.HEADER_LENGTH..LIMIT) { "native status frame exceeds bound" }
        // Reject compression before the general header decoder can allocate decompressed data.
        require(wire[22].toInt() == NoritoHeader.COMPRESSION_NONE &&
            wire[39].toInt() == NoritoHeader.COMPACT_LEN) { "native status requires the canonical declared layout" }
        val frame = NoritoHeader.decode(wire, SchemaHash.hash16(SCHEMA))
        require(frame.header.flags == NoritoHeader.COMPACT_LEN && frame.header.compression == NoritoHeader.COMPRESSION_NONE) { "native status requires the canonical declared layout" }
        val r = Reader(frame.payload)
        val json = linkedMapOf<String, Any?>(
            "protocol_version" to r.number(2),
            "config_fingerprint" to decodeHash(r.bytes(32)),
            "beacon_horizon" to r.optional(::decodeHorizon),
            "instance" to r.bytes(32).joinToString("") { "%02x".format(it.toInt() and 255) },
            "height" to r.number(8),
            "view" to r.number(8),
            "stage" to r.number(1),
            "leader" to r.optional(::decodeKey),
            "proxy_tail" to r.optional(::decodeKey),
            "high_qc_view" to r.optional { Reader(it).wholeNumber(8) },
            "level" to r.number(4),
            "start_level" to r.number(4),
            "t_retx_ms" to r.number(8),
            "committed_height" to r.number(8),
            "applied_height" to r.number(8),
            "awaiting" to r.bool(),
            "signer" to r.optional(::decodeKey),
            "unanchored" to r.bool(),
            "abstaining" to r.bool(),
            "halted" to r.optional(::decodeHalt),
            "footprint" to decodeFootprint(r.field())
        )
        r.finish()
        val value = SumeragiStatus.parseJson(JsonEncoder.encode(json))
        require(encode(value).contentEquals(wire)) { "non-canonical native status frame" }
        return value
    }
    private fun encodeFootprint(v: SumeragiFootprint) = record(integer(v.votes, 8), integer(v.timeouts, 8), integer(v.blocks, 8), integer(v.execEntries, 8), integer(v.wants, 8), integer(v.pendingApply, 8), integer(v.syncEntries, 8), integer(v.syncBytes, 8), integer(v.peers, 8), integer(v.recentHeaders, 8), integer(v.configs, 8), integer(v.certCache, 8), integer(v.evidenceKeys, 8), integer(v.probe, 8))
    private fun decodeFootprint(bytes: ByteArray): Map<String, Any?> {
        val r = Reader(bytes)
        val values = linkedMapOf<String, Any?>("votes" to r.number(8), "timeouts" to r.number(8), "blocks" to r.number(8), "exec_entries" to r.number(8), "wants" to r.number(8), "pending_apply" to r.number(8), "sync_entries" to r.number(8), "sync_bytes" to r.number(8), "peers" to r.number(8), "recent_headers" to r.number(8), "configs" to r.number(8), "cert_cache" to r.number(8), "evidence_keys" to r.number(8), "probe" to r.number(8))
        r.finish(); return values
    }
    private fun encodeHorizon(v: SumeragiBeaconHorizon) = record(integer(v.epochLengthBlocks, 8), option(v.nextRequiredPulseHeight?.let { integer(it, 8) }), option(v.activeSessionId?.let { record(*hex(it).map { byte -> byteArrayOf(byte) }.toTypedArray()) }), boolean(v.sessionCoversNextPulse), boolean(v.localProviderReady))
    private fun decodeHorizon(bytes: ByteArray): Map<String, Any?> {
        val r = Reader(bytes)
        val values = linkedMapOf<String, Any?>("epoch_length_blocks" to r.number(8),
            "next_required_pulse_height" to r.optional { Reader(it).wholeNumber(8) },
            "active_session_id" to r.optional {
                val array = Reader(it)
                val bytes = ByteArray(32) { array.bytes(1)[0] }
                array.finish()
                bytes.joinToString("") { byte -> "%02X".format(byte.toInt() and 255) }
            },
            "session_covers_next_pulse" to r.bool(), "local_provider_ready" to r.bool())
        r.finish(); return values
    }
    private val haltKinds = listOf(SumeragiHaltKind.SAFETY_RECORD_CORRUPT, SumeragiHaltKind.SAFETY_RECORD_INCONSISTENT, SumeragiHaltKind.SAFETY_VIOLATION, SumeragiHaltKind.APPLY_DIVERGED, SumeragiHaltKind.PUBLICATION_RECOVERY_REQUIRED, SumeragiHaltKind.DRIVER_ANOMALY)
    private fun encodeHalt(v: SumeragiHaltReason): ByteArray = integer(haltKinds.indexOf(v.kind).toBigInteger(), 4) + (v.height?.let { record(integer(it, 8)) } ?: byteArrayOf())
    private fun decodeHalt(bytes: ByteArray): Map<String, Any?> {
        val r = Reader(bytes)
        val index = r.raw(4).let { BigInteger(1, it.reversedArray()).intValueExact() }
        require(index in haltKinds.indices)
        val kind = haltKinds[index]
        val height = if (kind.hasHeight) r.number(8) else null
        r.finish(); return mapOf("reason" to kind.wireName, "details" to height)
    }
    private fun encodeKey(literal: String): ByteArray {
        val key = requireNotNull(decodePublicKeyLiteral(literal))
        val bytes = compactPublicKeyPayload(key.curveId, key.keyBytes)
        return integer(bytes.size.toBigInteger(), 8) + record(*bytes.map { byteArrayOf(it) }.toTypedArray())
    }
    private fun decodeKey(bytes: ByteArray): String {
        val r = Reader(bytes)
        val count = BigInteger(1, r.raw(8).reversedArray())
        require(count >= BigInteger.valueOf(2) && count <= BigInteger.valueOf(65_536) && count <= BigInteger.valueOf((r.remaining() / 2).toLong()))
        val payload = ByteArray(count.toInt()) { r.bytes(1)[0] }
        r.finish()
        val key = requireNotNull(decodeCompactPublicKeyPayload(payload))
        return encodePublicKeyMultihash(key.curveId, key.keyBytes)
    }
    private fun decodeHash(bytes: ByteArray): String {
        require(bytes.size == 32 && bytes[31].toInt() and 1 == 1)
        return HashLiteral.canonicalize(bytes)
    }
    private fun integer(v: BigInteger, size: Int): ByteArray {
        require(v.signum() >= 0 && v.bitLength() <= size * 8)
        return ByteArray(size) { i -> v.shiftRight(i * 8).toByte() }
    }
    private fun boolean(v: Boolean) = byteArrayOf(if (v) 1 else 0)
    private fun hex(v: String) = ByteArray(v.length / 2) { i -> v.substring(i * 2, i * 2 + 2).toInt(16).toByte() }
    private fun length(v: Int): ByteArray {
        require(v >= 0)
        var n = v
        val out = ByteArrayOutputStream()
        do { val b = n and 127; n = n ushr 7; out.write(b or if (n == 0) 0 else 128) } while (n != 0)
        return out.toByteArray()
    }
    private fun record(vararg fields: ByteArray): ByteArray {
        val out = ByteArrayOutputStream()
        for (f in fields) { out.write(length(f.size)); out.write(f) }
        return out.toByteArray()
    }
    private fun option(v: ByteArray?) = if (v == null) byteArrayOf(0) else byteArrayOf(1) + record(v)
    private class Reader(private val bytes: ByteArray) {
        private var cursor = 0
        fun remaining() = bytes.size - cursor
        fun raw(n: Int): ByteArray { require(n >= 0 && n <= remaining()); return bytes.copyOfRange(cursor, cursor + n).also { cursor += n } }
        fun finish() { require(remaining() == 0) { "trailing native status bytes" } }
        fun field(): ByteArray {
            var size = 0; var shift = 0; var count = 0
            while (true) {
                val b = raw(1)[0].toInt() and 255
                require(shift < 21) { "status field exceeds frame bound" }
                size = size or ((b and 127) shl shift); count++
                if ((b and 128) == 0) break
                shift += 7
            }
            require(length(size).size == count && size <= LIMIT)
            return raw(size)
        }
        fun bytes(n: Int): ByteArray = field().also { require(it.size == n) }
        fun number(n: Int): BigInteger = BigInteger(1, bytes(n).reversedArray())
        fun wholeNumber(n: Int): BigInteger = BigInteger(1, raw(n).reversedArray()).also { finish() }
        fun bool(): Boolean { val b = bytes(1)[0].toInt(); require(b == 0 || b == 1); return b == 1 }
        fun <T> optional(decode: (ByteArray) -> T): T? {
            val r = Reader(field()); val tag = r.raw(1)[0].toInt()
            val value = when (tag) { 0 -> null; 1 -> decode(r.field()); else -> error("invalid Option tag") }
            r.finish(); return value
        }
    }
}
