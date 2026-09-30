// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.consensus

import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction
import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.core.util.HashLiteral
import org.hyperledger.iroha.sdk.address.decodePublicKeyLiteral
import org.hyperledger.iroha.sdk.address.encodePublicKeyMultihash

/** Sole native consensus status revision. Older layouts are rejected. */
const val SUMERAGI_STATUS_PROTOCOL_VERSION: Int = 1
/** Maximum JSON response size for native status. */
const val SUMERAGI_STATUS_JSON_MAX_BYTES: Long = 1L * 1024L * 1024L

/** Immutable status value semantics shared by Kotlin and Java callers. */
abstract class SumeragiStatusValue internal constructor() {
    protected abstract fun equalityFields(): List<Any?>
    final override fun equals(other: Any?): Boolean = other != null && javaClass == other.javaClass &&
        equalityFields() == (other as SumeragiStatusValue).equalityFields()
    final override fun hashCode(): Int = 31 * javaClass.hashCode() + equalityFields().hashCode()
}

/** Exact unsigned native core memory counters; these are observations, not limits. */
class SumeragiFootprint internal constructor(
    @JvmField val votes: BigInteger,
    @JvmField val timeouts: BigInteger,
    @JvmField val blocks: BigInteger,
    @JvmField val execEntries: BigInteger,
    @JvmField val wants: BigInteger,
    @JvmField val pendingApply: BigInteger,
    @JvmField val syncEntries: BigInteger,
    @JvmField val syncBytes: BigInteger,
    @JvmField val peers: BigInteger,
    @JvmField val recentHeaders: BigInteger,
    @JvmField val configs: BigInteger,
    @JvmField val certCache: BigInteger,
    @JvmField val evidenceKeys: BigInteger,
    @JvmField val probe: BigInteger,
) : SumeragiStatusValue() {
    override fun equalityFields(): List<Any?> = listOf(votes, timeouts, blocks, execEntries, wants, pendingApply, syncEntries, syncBytes, peers, recentHeaders, configs, certCache, evidenceKeys, probe)
}

/** Readiness observed at this applied cut by the sole native beacon owner. */
class SumeragiBeaconHorizon internal constructor(
    @JvmField val epochLengthBlocks: BigInteger,
    @JvmField val nextRequiredPulseHeight: BigInteger?,
    @JvmField val activeSessionId: String?,
    @JvmField val sessionCoversNextPulse: Boolean,
    @JvmField val localProviderReady: Boolean,
) : SumeragiStatusValue() {
    override fun equalityFields(): List<Any?> = listOf(epochLengthBlocks, nextRequiredPulseHeight, activeSessionId, sessionCoversNextPulse, localProviderReady)
}

/** Native halt causes; only the three height-bearing cases carry a height. */
enum class SumeragiHaltKind(@JvmField val wireName: String, internal val hasHeight: Boolean) {
    SAFETY_RECORD_CORRUPT("safety_record_corrupt", false),
    SAFETY_RECORD_INCONSISTENT("safety_record_inconsistent", false),
    SAFETY_VIOLATION("safety_violation", true),
    APPLY_DIVERGED("apply_diverged", true),
    PUBLICATION_RECOVERY_REQUIRED("publication_recovery_required", true),
    DRIVER_ANOMALY("driver_anomaly", false),
}

/** One closed native halt reason with its exact optional height. */
class SumeragiHaltReason internal constructor(
    @JvmField val kind: SumeragiHaltKind,
    @JvmField val height: BigInteger?,
) : SumeragiStatusValue() {
    override fun equalityFields(): List<Any?> = listOf(kind, height)
}

/** Native instance status. This observation is not an authenticated finality proof. */
class SumeragiStatus internal constructor(
    @JvmField val protocolVersion: Int,
    @JvmField val configFingerprint: String,
    @JvmField val beaconHorizon: SumeragiBeaconHorizon?,
    @JvmField val instance: String,
    @JvmField val height: BigInteger,
    @JvmField val view: BigInteger,
    @JvmField val stage: Int,
    @JvmField val leader: String?,
    @JvmField val proxyTail: String?,
    @JvmField val highQcView: BigInteger?,
    @JvmField val level: BigInteger,
    @JvmField val startLevel: BigInteger,
    @JvmField val tRetxMs: BigInteger,
    @JvmField val committedHeight: BigInteger,
    @JvmField val appliedHeight: BigInteger,
    @JvmField val awaiting: Boolean,
    @JvmField val signer: String?,
    @JvmField val unanchored: Boolean,
    @JvmField val abstaining: Boolean,
    @JvmField val halted: SumeragiHaltReason?,
    @JvmField val footprint: SumeragiFootprint,
) : SumeragiStatusValue() {
    override fun equalityFields(): List<Any?> = listOf(protocolVersion, configFingerprint, beaconHorizon, instance, height, view, stage, leader, proxyTail, highQcView, level, startLevel, tRetxMs, committedHeight, appliedHeight, awaiting, signer, unanchored, abstaining, halted, footprint)

    fun isHalted(): Boolean = halted != null
    fun isSigning(): Boolean = signer != null && !abstaining && !unanchored
    fun applyLag(): BigInteger = (committedHeight - appliedHeight).max(BigInteger.ZERO)

    companion object {
        /** Strict bounded UTF-8 JSON with required nullable fields and no unknown fields. */
        @JvmStatic
        fun parseJson(payload: ByteArray): SumeragiStatus {
            require(payload.isNotEmpty() && payload.size.toLong() <= SUMERAGI_STATUS_JSON_MAX_BYTES) {
                "Sumeragi status response is empty or exceeds its byte limit"
            }
            return NativeStatusParser.parse(SumeragiJsonPrimitives.decodeUtf8(payload, "Sumeragi status"))
        }
        /** Strict JSON decoding; exact unsigned values never pass through floating point. */
        @JvmStatic
        fun parseJson(payload: String): SumeragiStatus = parseJson(payload.toByteArray(StandardCharsets.UTF_8))
    }
}

internal object NativeStatusParser {
    private val fields = setOf("protocol_version","config_fingerprint","beacon_horizon","instance","height","view","stage","leader","proxy_tail","high_qc_view","level","start_level","t_retx_ms","committed_height","applied_height","awaiting","signer","unanchored","abstaining","halted","footprint")
    private val footprintFields = setOf("votes","timeouts","blocks","exec_entries","wants","pending_apply","sync_entries","sync_bytes","peers","recent_headers","configs","cert_cache","evidence_keys","probe")
    private fun key(value: Any?, name: String): String? {
        if (value == null) return null
        require(value is String) { "$name must be a canonical public key or null" }
        val key = requireNotNull(decodePublicKeyLiteral(value)) { "$name is not an admitted public key" }
        require(encodePublicKeyMultihash(key.curveId, key.keyBytes) == value) { "$name must be canonical" }
        return value
    }
    private fun halt(value: Any?): SumeragiHaltReason? {
        if (value == null) return null
        val record = SumeragiJsonPrimitives.exactObject(value, setOf("reason", "details"), "halted")
        val kind = requireNotNull(SumeragiHaltKind.values().find { it.wireName == record["reason"] }) {
            "unknown native halt reason"
        }
        val height = if (kind.hasHeight) SumeragiJsonPrimitives.u64(record["details"], "halted.details") else {
            require(record["details"] == null) { "unit halt reason requires explicit null details" }
            null
        }
        return SumeragiHaltReason(kind, height)
    }
    private fun horizon(value: Any?): SumeragiBeaconHorizon? {
        if (value == null) return null
        val r = SumeragiJsonPrimitives.exactObject(value, setOf(
            "epoch_length_blocks", "next_required_pulse_height", "active_session_id",
            "session_covers_next_pulse", "local_provider_ready"), "beacon_horizon")
        val next = r["next_required_pulse_height"]?.let { SumeragiJsonPrimitives.u64(it, "next_required_pulse_height") }
        val session = r["active_session_id"]?.let { SumeragiJsonPrimitives.byte32(it, "active_session_id") }
        val covers = SumeragiJsonPrimitives.boolean(r["session_covers_next_pulse"], "session_covers_next_pulse")
        val ready = SumeragiJsonPrimitives.boolean(r["local_provider_ready"], "local_provider_ready")
        require(!covers || (next != null && session != null)) { "coverage requires a session and a demand" }
        require(!ready || session != null) { "provider readiness requires an installed session" }
        return SumeragiBeaconHorizon(SumeragiJsonPrimitives.u64(r["epoch_length_blocks"], "epoch_length_blocks"), next, session, covers, ready)
    }
    fun parse(payload: String): SumeragiStatus =
        parseValue(SumeragiJsonPrimitives.parseObject(payload, "native status"))

    /** Validates one already-parsed status object, such as a lane instance's status. */
    fun parseValue(value: Any?): SumeragiStatus {
        val r = SumeragiJsonPrimitives.exactObject(value, fields, "native status")
        fun u64(n: String) = SumeragiJsonPrimitives.u64(r[n], n)
        fun u32(n: String) = SumeragiJsonPrimitives.u32(r[n], n)
        fun bool(n: String) = SumeragiJsonPrimitives.boolean(r[n], n)
        val version = SumeragiJsonPrimitives.u16(r["protocol_version"], "protocol_version")
        require(version == SUMERAGI_STATUS_PROTOCOL_VERSION) { "unsupported native status protocol" }
        val instance = r["instance"]
        require(instance is String && Regex("^[0-9a-f]{64}$").matches(instance)) { "instance must be lowercase 32-byte hex" }
        val stage = SumeragiJsonPrimitives.u16(r["stage"], "stage")
        require(stage <= 2) { "native routing stage must be 0, 1 or 2" }
        val f = SumeragiJsonPrimitives.exactObject(r["footprint"], footprintFields, "footprint")
        val footprint = SumeragiFootprint(SumeragiJsonPrimitives.u64(f["votes"], "footprint.votes"), SumeragiJsonPrimitives.u64(f["timeouts"], "footprint.timeouts"), SumeragiJsonPrimitives.u64(f["blocks"], "footprint.blocks"), SumeragiJsonPrimitives.u64(f["exec_entries"], "footprint.exec_entries"), SumeragiJsonPrimitives.u64(f["wants"], "footprint.wants"), SumeragiJsonPrimitives.u64(f["pending_apply"], "footprint.pending_apply"), SumeragiJsonPrimitives.u64(f["sync_entries"], "footprint.sync_entries"), SumeragiJsonPrimitives.u64(f["sync_bytes"], "footprint.sync_bytes"), SumeragiJsonPrimitives.u64(f["peers"], "footprint.peers"), SumeragiJsonPrimitives.u64(f["recent_headers"], "footprint.recent_headers"), SumeragiJsonPrimitives.u64(f["configs"], "footprint.configs"), SumeragiJsonPrimitives.u64(f["cert_cache"], "footprint.cert_cache"), SumeragiJsonPrimitives.u64(f["evidence_keys"], "footprint.evidence_keys"), SumeragiJsonPrimitives.u64(f["probe"], "footprint.probe"))
        return SumeragiStatus(
            version, SumeragiJsonPrimitives.hash(r["config_fingerprint"], "config_fingerprint", true),
            horizon(r["beacon_horizon"]), instance, u64("height"), u64("view"), stage,
            key(r["leader"], "leader"), key(r["proxy_tail"], "proxy_tail"),
            r["high_qc_view"]?.let { SumeragiJsonPrimitives.u64(it, "high_qc_view") },
            u32("level"), u32("start_level"), u64("t_retx_ms"), u64("committed_height"),
            u64("applied_height"), bool("awaiting"), key(r["signer"], "signer"),
            bool("unanchored"), bool("abstaining"), halt(r["halted"]), footprint,
        )
    }
}

/** Shared strict scalar rules used by authoritative status and operational diagnostics. */
internal object SumeragiJsonPrimitives {
    private val u64Max = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
    private val u32Max = BigInteger.ONE.shiftLeft(32).subtract(BigInteger.ONE)
    private val canonicalHash = Regex("^hash:[0-9A-F]{64}#[0-9A-F]{4}$")
    private val canonicalByte32 = Regex("^[0-9A-F]{64}$")

    fun decodeUtf8(payload: ByteArray, context: String): String {
        val decoder = StandardCharsets.UTF_8.newDecoder()
            .onMalformedInput(CodingErrorAction.REPORT)
            .onUnmappableCharacter(CodingErrorAction.REPORT)
        return try {
            decoder.decode(ByteBuffer.wrap(payload)).toString()
        } catch (error: Exception) {
            throw IllegalArgumentException("$context must be valid UTF-8", error)
        }
    }

    fun parseObject(payload: String, context: String): Map<String, Any?> =
        objectValue(parseValue(payload, context), context)

    /** Strict JSON with unique keys and no negative-zero tokens; the root may be any value. */
    fun parseValue(payload: String, context: String): Any? {
        rejectNegativeZeroTokens(payload, context)
        return try {
            JsonParser.parse(payload)
        } catch (error: IllegalStateException) {
            throw IllegalArgumentException("$context must be valid JSON with unique keys", error)
        }
    }

    @Suppress("UNCHECKED_CAST")
    fun objectValue(value: Any?, context: String): Map<String, Any?> {
        require(value is Map<*, *>) { "$context must be a JSON object" }
        require(value.keys.all { it is String }) { "$context contains a non-string field name" }
        return value as Map<String, Any?>
    }

    fun exactObject(
        value: Any?,
        fields: Set<String>,
        context: String,
    ): Map<String, Any?> {
        val record = objectValue(value, context)
        requireFields(record, fields, emptySet(), context)
        return record
    }

    fun requireFields(
        record: Map<String, Any?>,
        required: Set<String>,
        optional: Set<String>,
        context: String,
    ) {
        val allowed = required + optional
        val unknown = record.keys.firstOrNull { it !in allowed }
        require(unknown == null) { "$context contains unknown field $unknown" }
        val missing = required.firstOrNull { !record.containsKey(it) }
        require(missing == null) { "$context is missing required field $missing" }
    }

    fun array(value: Any?, context: String, maximum: Int): List<Any?> {
        require(value is List<*>) { "$context must be a JSON array" }
        require(value.size <= maximum) { "$context exceeds its protocol item bound" }
        return value
    }

    fun unsigned(
        value: Any?,
        maximum: BigInteger,
        context: String,
        positive: Boolean = false,
    ): BigInteger {
        val parsed = when (value) {
            is Long -> BigInteger.valueOf(value)
            is BigInteger -> value
            else -> throw IllegalArgumentException("$context must be an unquoted integer")
        }
        require(parsed.signum() >= 0) { "$context must be non-negative" }
        require(!positive || parsed.signum() > 0) { "$context must be positive" }
        require(parsed <= maximum) { "$context exceeds its protocol bound" }
        return parsed
    }

    fun u64(value: Any?, context: String): BigInteger = unsigned(value, u64Max, context)

    fun positiveU64(value: Any?, context: String): BigInteger =
        unsigned(value, u64Max, context, true)

    fun u32(value: Any?, context: String): BigInteger = unsigned(value, u32Max, context)

    fun positiveU32(value: Any?, context: String): BigInteger =
        unsigned(value, u32Max, context, true)

    fun u16(value: Any?, context: String): Int =
        unsigned(value, BigInteger.valueOf(0xffff), context).toInt()

    fun boolean(value: Any?, context: String): Boolean {
        require(value is Boolean) { "$context must be a boolean" }
        return value
    }

    fun hash(value: Any?, context: String, nonzero: Boolean = false): String {
        require(value is String && canonicalHash.matches(value)) {
            "$context must be a canonical Iroha hash literal"
        }
        val bytes = try {
            HashLiteral.decode(value)
        } catch (error: IllegalArgumentException) {
            throw IllegalArgumentException("$context must have a valid canonical checksum", error)
        }
        require((bytes[bytes.lastIndex].toInt() and 1) == 1) {
            "$context has an invalid Iroha hash marker bit"
        }
        require(!nonzero || bytes.any { it.toInt() != 0 }) { "$context must not be the zero hash" }
        return value
    }

    fun byte32(value: Any?, context: String): String {
        require(value is String && canonicalByte32.matches(value)) {
            "$context must be canonical uppercase 32-byte hex"
        }
        return value
    }

    fun taggedUnit(value: Any?, tag: String, context: String): String {
        val record = exactObject(value, setOf(tag, "details"), context)
        val variant = record[tag]
        require(variant is String && variant.isNotEmpty()) { "$context.$tag must be a string" }
        require(record["details"] == null) { "$context.details must be explicitly null" }
        return variant
    }

    fun requireU64(value: BigInteger, context: String): BigInteger {
        require(value.signum() >= 0 && value <= u64Max) {
            "$context must fit in an unsigned 64-bit integer"
        }
        return value
    }

    fun requireCanonicalNonzeroHash(value: String, context: String) {
        hash(value, context, nonzero = true)
    }

    private fun rejectNegativeZeroTokens(payload: String, context: String) {
        var inString = false
        var escaped = false
        var index = 0
        while (index < payload.length) {
            val current = payload[index]
            if (inString) {
                if (escaped) {
                    escaped = false
                } else if (current == '\\') {
                    escaped = true
                } else if (current == '"') {
                    inString = false
                }
                index += 1
                continue
            }
            if (current == '"') {
                inString = true
                index += 1
                continue
            }
            if (current == '-' && index + 1 < payload.length && payload[index + 1] == '0') {
                val after = payload.getOrNull(index + 2)
                if (after == null || after in " \t\r\n,]}") {
                    throw IllegalArgumentException("$context contains noncanonical negative zero")
                }
            }
            index += 1
        }
    }
}
