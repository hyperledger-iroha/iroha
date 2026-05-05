package org.hyperledger.iroha.sdk.offline

import org.hyperledger.iroha.sdk.client.JsonParser
import java.io.IOException
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.StandardOpenOption

/**
 * File-backed audit log that mirrors the iOS SDK toggle. Entries are persisted as a JSON array so
 * operators can export deterministic payloads for regulators.
 */
class OfflineAuditLogger @Throws(IOException::class) constructor(
    val logFile: Path,
    @Volatile var isEnabled: Boolean,
) {
    private val lock = Any()
    private var _entries: MutableList<OfflineAuditEntry>

    init {
        val parent = logFile.parent
        if (parent != null) {
            Files.createDirectories(parent)
        }
        _entries = ArrayList()
        if (Files.exists(logFile) && Files.size(logFile) > 0) {
            loadExisting()
        }
    }

    @Throws(IOException::class)
    fun record(entry: OfflineAuditEntry) {
        if (!isEnabled) return
        synchronized(lock) {
            _entries.add(entry)
            persist()
        }
    }

    fun entries(): List<OfflineAuditEntry> {
        synchronized(lock) {
            return _entries.toList()
        }
    }

    @Throws(IOException::class)
    fun exportJson(): ByteArray {
        synchronized(lock) {
            return encodeEntries(_entries).toByteArray(Charsets.UTF_8)
        }
    }

    @Throws(IOException::class)
    fun clear() {
        synchronized(lock) {
            _entries.clear()
            persist()
        }
    }

    @Suppress("UNCHECKED_CAST")
    @Throws(IOException::class)
    private fun loadExisting() {
        val json = String(Files.readAllBytes(logFile), Charsets.UTF_8).trim()
        if (json.isEmpty()) {
            _entries = ArrayList()
            return
        }
        val parsed = JsonParser.parse(json)
        if (parsed !is List<*>) {
            throw IOException("Audit log is corrupted (expected JSON array)")
        }
        val restored = ArrayList<OfflineAuditEntry>(parsed.size)
        for (element in parsed) {
            if (element !is Map<*, *>) {
                throw IOException("Audit log contains non-object entries")
            }
            restored.add(OfflineAuditEntry.fromJsonMap(element as Map<String, Any>))
        }
        _entries = restored
    }

    @Throws(IOException::class)
    private fun persist() {
        val json = encodeEntries(_entries)
        Files.write(
            logFile,
            json.toByteArray(Charsets.UTF_8),
            StandardOpenOption.CREATE,
            StandardOpenOption.TRUNCATE_EXISTING,
        )
    }

    companion object {
        @Suppress("UNCHECKED_CAST")
        private fun encodeEntries(entries: List<OfflineAuditEntry>): String {
            val builder = StringBuilder()
            builder.append('[')
            for (i in entries.indices) {
                if (i > 0) builder.append(',')
                writeObject(builder, entries[i].toJson())
            }
            builder.append(']')
            return builder.toString()
        }

        private fun writeObject(builder: StringBuilder, map: Map<String, Any>) {
            builder.append('{')
            var first = true
            for ((key, value) in map) {
                if (!first) {
                    builder.append(',')
                } else {
                    first = false
                }
                writeString(builder, key)
                builder.append(':')
                writeValue(builder, value)
            }
            builder.append('}')
        }

        @Suppress("UNCHECKED_CAST")
        private fun writeValue(builder: StringBuilder, value: Any?) {
            when (value) {
                null -> builder.append("null")
                is String -> writeString(builder, value)
                is Number -> builder.append(value.toString())
                is Boolean -> builder.append(if (value) "true" else "false")
                is Map<*, *> -> writeObject(builder, value as Map<String, Any>)
                else -> throw IllegalArgumentException("Unsupported JSON value: $value")
            }
        }

        private fun writeString(builder: StringBuilder, value: String) {
            builder.append('"')
            for (c in value) {
                when (c) {
                    '"' -> builder.append("\\\"")
                    '\\' -> builder.append("\\\\")
                    '\b' -> builder.append("\\b")
                    '\u000C' -> builder.append("\\f")
                    '\n' -> builder.append("\\n")
                    '\r' -> builder.append("\\r")
                    '\t' -> builder.append("\\t")
                    else -> {
                        if (c < '\u0020') {
                            builder.append(String.format("\\u%04x", c.code))
                        } else {
                            builder.append(c)
                        }
                    }
                }
            }
            builder.append('"')
        }
    }
}
