package org.hyperledger.iroha.sdk.offline

import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.crypto.Blake2b
import java.io.ByteArrayOutputStream
import java.io.IOException
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.StandardOpenOption
import java.time.Instant
import java.util.TreeMap

private const val PROVISIONED_PREFIX = "provisioned::"

/** File-backed store for offline platform counters and summary hashes. */
class OfflineCounterJournal @Throws(IOException::class) constructor(
    val journalFile: Path,
) {
    private val lock = Any()
    private val entries = LinkedHashMap<String, OfflineCounterCheckpoint>()

    init {
        val parent = journalFile.parent
        if (parent != null) {
            Files.createDirectories(parent)
        }
        if (Files.exists(journalFile) && Files.size(journalFile) > 0) {
            loadExisting()
        }
    }

    fun entries(): List<OfflineCounterCheckpoint> {
        synchronized(lock) {
            return entries.values.toList()
        }
    }

    fun find(certificateIdHex: String?): OfflineCounterCheckpoint? {
        if (certificateIdHex.isNullOrBlank()) return null
        synchronized(lock) {
            val direct = entries[certificateIdHex]
            if (direct != null) return direct
            val normalized = certificateIdHex.trim()
            for ((key, value) in entries) {
                if (key.equals(normalized, ignoreCase = true)) return value
            }
            return null
        }
    }

    /** Exports the stored checkpoints as canonical JSON. */
    fun exportJson(): ByteArray {
        synchronized(lock) {
            return JsonEncoder.encode(serializeEntriesLocked()).toByteArray(Charsets.UTF_8)
        }
    }

    @Throws(IOException::class)
    fun upsert(
        summaryList: OfflineSummaryList,
        recordedAt: Instant,
    ): List<OfflineCounterCheckpoint> =
        upsert(summaryList.items, recordedAt)

    @Throws(IOException::class)
    fun upsert(
        summaries: List<OfflineSummaryList.OfflineSummaryItem>,
        recordedAt: Instant,
    ): List<OfflineCounterCheckpoint> {
        val recordedAtMs = recordedAt.toEpochMilli()
        synchronized(lock) {
            val inserted = ArrayList<OfflineCounterCheckpoint>(summaries.size)
            for (summary in summaries) {
                val certificateId = normalizeHex(summary.certificateIdHex)
                val computed = computeSummaryHashHex(summary.appleKeyCounters, summary.androidSeriesCounters)
                val expected = normalizeHex(summary.summaryHashHex)
                if (!computed.equals(expected, ignoreCase = true)) {
                    throw OfflineCounterException(
                        OfflineCounterException.Reason.SUMMARY_HASH_MISMATCH,
                        "summary hash mismatch for $certificateId: expected $expected, got $computed",
                    )
                }
                val checkpoint = OfflineCounterCheckpoint(
                    certificateId,
                    summary.controllerId,
                    summary.controllerDisplay,
                    computed,
                    summary.appleKeyCounters,
                    summary.androidSeriesCounters,
                    recordedAtMs,
                )
                entries[certificateId] = checkpoint
                inserted.add(checkpoint)
            }
            persistLocked()
            return inserted
        }
    }

    @Throws(IOException::class)
    fun advanceAppleCounter(
        certificateIdHex: String,
        controllerId: String,
        controllerDisplay: String?,
        keyId: String,
        counter: Long,
        recordedAt: Instant,
    ): OfflineCounterCheckpoint =
        advanceCounter(
            certificateIdHex, controllerId, controllerDisplay,
            OfflineCounterPlatform.APPLE_KEY, keyId, counter, recordedAt,
        )

    @Throws(IOException::class)
    fun advanceAndroidSeriesCounter(
        certificateIdHex: String,
        controllerId: String,
        controllerDisplay: String?,
        series: String,
        counter: Long,
        recordedAt: Instant,
    ): OfflineCounterCheckpoint =
        advanceCounter(
            certificateIdHex, controllerId, controllerDisplay,
            OfflineCounterPlatform.ANDROID_SERIES, series, counter, recordedAt,
        )

    @Throws(IOException::class)
    fun advanceProvisionedCounter(
        certificateIdHex: String,
        controllerId: String,
        controllerDisplay: String?,
        proof: AndroidProvisionedProof,
        recordedAt: Instant,
    ): OfflineCounterCheckpoint {
        val schema = proof.manifestSchema
        val deviceId = proof.deviceId
        if (schema.isNullOrBlank() || deviceId.isNullOrBlank()) {
            throw OfflineCounterException(
                OfflineCounterException.Reason.INVALID_SCOPE,
                "provisioned manifest schema/device_id missing",
            )
        }
        val scope = "$PROVISIONED_PREFIX${schema.trim()}::${deviceId.trim()}"
        return advanceCounter(
            certificateIdHex, controllerId, controllerDisplay,
            OfflineCounterPlatform.ANDROID_SERIES, scope, proof.counter, recordedAt,
        )
    }

    @Throws(IOException::class)
    private fun advanceCounter(
        certificateIdHex: String,
        controllerId: String,
        controllerDisplay: String?,
        platform: OfflineCounterPlatform,
        scope: String?,
        counter: Long,
        recordedAt: Instant,
    ): OfflineCounterCheckpoint {
        val normalizedCert = normalizeHex(certificateIdHex)
        val trimmedScope = scope?.trim() ?: ""
        if (normalizedCert.isBlank()) {
            throw OfflineCounterException(
                OfflineCounterException.Reason.INVALID_SCOPE,
                "certificate_id_hex must not be empty",
            )
        }
        if (trimmedScope.isEmpty()) {
            throw OfflineCounterException(
                OfflineCounterException.Reason.INVALID_SCOPE,
                "counter scope must not be empty",
            )
        }
        if (counter < 0) {
            throw OfflineCounterException(
                OfflineCounterException.Reason.INVALID_SCOPE,
                "counter must be non-negative",
            )
        }
        val recordedAtMs = recordedAt.toEpochMilli()
        synchronized(lock) {
            val existing = entries[normalizedCert]
            val apple = if (existing == null) LinkedHashMap() else LinkedHashMap(existing.appleKeyCounters)
            val android = if (existing == null) LinkedHashMap() else LinkedHashMap(existing.androidSeriesCounters)
            val target = if (platform == OfflineCounterPlatform.APPLE_KEY) apple else android
            if (target.containsKey(trimmedScope)) {
                val previous = target[trimmedScope]!!
                val expected = previous + 1
                if (counter != expected) {
                    throw OfflineCounterException(
                        OfflineCounterException.Reason.COUNTER_VIOLATION,
                        "counter jump for $trimmedScope: expected $expected, got $counter",
                    )
                }
            }
            target[trimmedScope] = counter
            val summaryHash = computeSummaryHashHex(apple, android)
            val resolvedControllerId =
                if (existing != null && !existing.controllerId.isNullOrBlank())
                    existing.controllerId
                else controllerId
            val resolvedControllerDisplay =
                if (existing != null && !existing.controllerDisplay.isNullOrBlank())
                    existing.controllerDisplay
                else controllerDisplay
            val checkpoint = OfflineCounterCheckpoint(
                normalizedCert,
                resolvedControllerId,
                resolvedControllerDisplay,
                summaryHash,
                apple,
                android,
                recordedAtMs,
            )
            entries[normalizedCert] = checkpoint
            persistLocked()
            return checkpoint
        }
    }

    @Suppress("UNCHECKED_CAST")
    @Throws(IOException::class)
    private fun loadExisting() {
        val json = String(Files.readAllBytes(journalFile), Charsets.UTF_8).trim()
        if (json.isEmpty()) return
        val root = JsonParser.parse(json)
        if (root !is Map<*, *>) {
            throw IOException("Counter journal must be a JSON object")
        }
        entries.clear()
        for ((key, value) in root) {
            val certificateId = key?.toString() ?: continue
            val checkpoint = OfflineCounterCheckpoint.fromJson(certificateId, value)
            entries[certificateId] = checkpoint
        }
    }

    @Throws(IOException::class)
    private fun persistLocked() {
        val serialized = serializeEntriesLocked()
        Files.write(
            journalFile,
            JsonEncoder.encode(serialized).toByteArray(Charsets.UTF_8),
            StandardOpenOption.CREATE,
            StandardOpenOption.TRUNCATE_EXISTING,
        )
    }

    private fun serializeEntriesLocked(): Map<String, Any> {
        val serialized = LinkedHashMap<String, Any>(entries.size)
        for ((key, value) in entries) {
            serialized[key] = value.toJson()
        }
        return serialized
    }

    private enum class OfflineCounterPlatform {
        APPLE_KEY,
        ANDROID_SERIES,
    }

    class OfflineCounterCheckpoint(
        val certificateIdHex: String?,
        val controllerId: String?,
        val controllerDisplay: String?,
        val summaryHashHex: String?,
        appleKeyCounters: Map<String, Long>,
        androidSeriesCounters: Map<String, Long>,
        val recordedAtMs: Long,
    ) {
        private val _appleKeyCounters: Map<String, Long> = appleKeyCounters.toMap()
        private val _androidSeriesCounters: Map<String, Long> = androidSeriesCounters.toMap()

        val appleKeyCounters: Map<String, Long> get() = _appleKeyCounters
        val androidSeriesCounters: Map<String, Long> get() = _androidSeriesCounters

        internal fun toJson(): Map<String, Any?> {
            val map = LinkedHashMap<String, Any?>()
            map["controller_id"] = controllerId
            map["controller_display"] = controllerDisplay
            map["summary_hash_hex"] = summaryHashHex
            map["apple_key_counters"] = _appleKeyCounters
            map["android_series_counters"] = _androidSeriesCounters
            map["recorded_at_ms"] = recordedAtMs
            return map
        }

        companion object {
            @JvmStatic
            fun fromJson(certificateIdHex: String, value: Any?): OfflineCounterCheckpoint {
                check(value is Map<*, *>) { "counter entry for $certificateIdHex is not an object" }
                val controllerId = value["controller_id"]?.toString() ?: ""
                val controllerDisplay = value["controller_display"]?.toString()
                val summaryHash = value["summary_hash_hex"]?.toString() ?: ""
                val apple = if (value["apple_key_counters"] is Map<*, *>)
                    asCounterMap(value["apple_key_counters"] as Map<*, *>)
                else emptyMap()
                val android = if (value["android_series_counters"] is Map<*, *>)
                    asCounterMap(value["android_series_counters"] as Map<*, *>)
                else emptyMap()
                val recordedAtMs = optionalLong(value["recorded_at_ms"], "recorded_at_ms")
                return OfflineCounterCheckpoint(
                    normalizeHex(certificateIdHex),
                    controllerId,
                    controllerDisplay,
                    summaryHash,
                    apple,
                    android,
                    recordedAtMs,
                )
            }

            private fun asCounterMap(map: Map<*, *>): Map<String, Long> {
                val result = LinkedHashMap<String, Long>()
                for ((key, value) in map) {
                    if (key == null || value == null) continue
                    result[key.toString()] = requireLong(value, "counter.$key")
                }
                return result
            }

            private fun optionalLong(value: Any?, path: String): Long {
                if (value == null) return 0L
                return requireLong(value, path)
            }

            private fun requireLong(value: Any, path: String): Long {
                if (value is Number) {
                    check(value !is Float && value !is Double) { "$path must be an integer" }
                    return value.toLong()
                }
                try {
                    return value.toString().toLong()
                } catch (ex: NumberFormatException) {
                    throw IllegalStateException("$path is not numeric", ex)
                }
            }
        }
    }

    companion object {
        private fun normalizeHex(value: String?): String {
            if (value == null) return ""
            var trimmed = value.trim()
            if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
                trimmed = trimmed.substring(2)
            }
            return trimmed.lowercase()
        }

        private fun computeSummaryHashHex(
            apple: Map<String, Long>,
            android: Map<String, Long>,
        ): String {
            val payload = encodeSummaryPayload(apple, android)
            val digest = irohaHash(payload)
            return toHex(digest)
        }

        private fun irohaHash(payload: ByteArray): ByteArray {
            val digest = Blake2b.digest256(payload)
            digest[digest.size - 1] = (digest[digest.size - 1].toInt() or 1).toByte()
            return digest
        }

        private fun encodeSummaryPayload(
            apple: Map<String, Long>,
            android: Map<String, Long>,
        ): ByteArray {
            val applePayload = encodeCounterMap(apple)
            val androidPayload = encodeCounterMap(android)
            val out = ByteArrayOutputStream()
            writeU64(out, applePayload.size.toLong())
            out.write(applePayload)
            writeU64(out, androidPayload.size.toLong())
            out.write(androidPayload)
            return out.toByteArray()
        }

        private fun encodeCounterMap(map: Map<String, Long>): ByteArray {
            val sorted = TreeMap(map)
            val out = ByteArrayOutputStream()
            writeU64(out, sorted.size.toLong())
            for ((key, value) in sorted) {
                val keyPayload = encodeString(key)
                writeU64(out, keyPayload.size.toLong())
                out.write(keyPayload)
                val valuePayload = encodeU64(value)
                writeU64(out, valuePayload.size.toLong())
                out.write(valuePayload)
            }
            return out.toByteArray()
        }

        private fun encodeString(value: String): ByteArray {
            val bytes = value.toByteArray(Charsets.UTF_8)
            val out = ByteArrayOutputStream()
            writeU64(out, bytes.size.toLong())
            out.write(bytes)
            return out.toByteArray()
        }

        private fun encodeU64(value: Long): ByteArray {
            val out = ByteArray(8)
            var v = value
            for (i in 0 until 8) {
                out[i] = (v and 0xFF).toByte()
                v = v ushr 8
            }
            return out
        }

        private fun writeU64(out: ByteArrayOutputStream, value: Long) {
            out.write(encodeU64(value))
        }

        private fun toHex(data: ByteArray): String {
            val builder = StringBuilder(data.size * 2)
            for (b in data) {
                builder.append(String.format("%02x", b))
            }
            return builder.toString()
        }
    }
}
