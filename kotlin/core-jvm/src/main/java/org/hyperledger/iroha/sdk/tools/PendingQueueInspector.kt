package org.hyperledger.iroha.sdk.tools

import java.io.IOException
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Path
import java.time.Instant
import java.time.ZoneOffset
import java.time.format.DateTimeFormatter
import java.util.Locale
import org.hyperledger.iroha.sdk.client.queue.FilePendingTransactionQueue
import org.hyperledger.iroha.sdk.tx.SignedTransaction
import org.hyperledger.iroha.sdk.tx.SignedTransactionHasher
import org.hyperledger.iroha.sdk.tx.offline.OfflineSigningEnvelope
import org.hyperledger.iroha.sdk.tx.offline.OfflineSigningEnvelopeCodec

private val DECODER = java.util.Base64.getDecoder()
private val ENVELOPE_CODEC = OfflineSigningEnvelopeCodec()
private val ISO_FORMATTER = DateTimeFormatter.ISO_OFFSET_DATE_TIME.withLocale(Locale.ROOT)
private const val DEFAULT_ALIAS = "pending.queue"

/**
 * Utility that inspects a pending queue file produced by `FilePendingTransactionQueue`. Useful
 * during AND7 queue-replay drills where operators must capture hashed payloads and alias metadata
 * for incident artefacts.
 */
object PendingQueueInspector {

    @JvmStatic
    fun main(args: Array<String>) {
        val arguments = Arguments.parse(args)
        if (arguments.queueFile == null) {
            printUsage()
            System.exit(1)
            return
        }
        val entries = inspect(arguments.queueFile)
        if (arguments.jsonOutput) {
            println(toJson(entries))
        } else {
            printHuman(entries)
        }
    }

    /** Inspect a queue file and return decoded entry summaries (exposed for tests). */
    @JvmStatic
    @Throws(IOException::class)
    fun inspect(queueFile: Path): List<EntrySummary> {
        require(Files.exists(queueFile)) { "queue file does not exist: $queueFile" }
        val lines = Files.readAllLines(queueFile, StandardCharsets.UTF_8)
        var index = 0
        return lines.filter { !it.isNullOrBlank() }.map { line ->
            decodeLine(line.trim(), index++)
        }
    }

    @Throws(IOException::class)
    private fun decodeLine(line: String, index: Int): EntrySummary {
        val bytes: ByteArray
        try {
            bytes = DECODER.decode(line)
        } catch (ex: IllegalArgumentException) {
            throw IOException("Failed to decode queue entry $index", ex)
        }
        try {
            val envelope = ENVELOPE_CODEC.decode(bytes)
            val transaction = SignedTransaction(
                envelope.encodedPayload,
                envelope.signature,
                envelope.publicKey,
                envelope.schemaName,
                envelope.keyAlias,
                envelope.exportedKeyBundle,
            )
            val issuedAtMs = if (envelope.issuedAtMs >= 0) envelope.issuedAtMs else null
            return EntrySummary(
                index = index,
                hashHex = SignedTransactionHasher.hashHex(transaction),
                schemaName = transaction.schemaName(),
                keyAlias = transaction.keyAlias().orElse(DEFAULT_ALIAS),
                issuedAtMs = issuedAtMs,
                hasExportedKeyBundle = envelope.exportedKeyBundle != null,
            )
        } catch (ex: Exception) {
            throw IOException("Failed to decode queue entry $index", ex)
        }
    }

    private fun printUsage() {
        System.err.println("""
Usage: PendingQueueInspector --file <path> [--json]

Options:
  --file <path>   Path to the pending queue file (required)
  --json          Emit JSON array instead of human-readable summary

Example:
  java -cp build/classes org.hyperledger.iroha.sdk.tools.PendingQueueInspector \
      --file /tmp/pixel8/pending.queue --json
""".trimIndent())
    }

    private fun printHuman(entries: List<EntrySummary>) {
        System.out.printf("Queue entries: %d%n", entries.size)
        for (entry in entries) {
            System.out.printf(
                Locale.ROOT,
                "[%d] hash=%s schema=%s alias=%s issued_at=%s exported_key_bundle=%s%n",
                entry.index,
                entry.hashHex,
                entry.schemaName,
                entry.keyAlias,
                entry.issuedAtIso() ?: "unknown",
                entry.hasExportedKeyBundle,
            )
        }
    }

    private fun toJson(entries: List<EntrySummary>): String = buildString {
        append('[')
        for (i in entries.indices) {
            if (i > 0) append(',')
            append(entries[i].toJson())
        }
        append(']')
    }

    /** Summary metadata for a queued transaction. */
    class EntrySummary(
        @JvmField val index: Int,
        @JvmField val hashHex: String,
        @JvmField val schemaName: String,
        @JvmField val keyAlias: String,
        @JvmField val issuedAtMs: Long?,
        @JvmField val hasExportedKeyBundle: Boolean,
    ) {
        fun issuedAtIso(): String? {
            if (issuedAtMs == null) return null
            val instant = Instant.ofEpochMilli(issuedAtMs)
            return ISO_FORMATTER.format(instant.atOffset(ZoneOffset.UTC))
        }

        internal fun toJson(): String = buildString {
            append('{')
            append("\"index\":").append(index).append(',')
            append("\"hash_hex\":\"").append(hashHex).append("\",")
            append("\"schema\":\"").append(escape(schemaName)).append("\",")
            append("\"alias\":\"").append(escape(keyAlias)).append("\",")
            append("\"issued_at_ms\":")
            if (issuedAtMs != null) append(issuedAtMs) else append("null")
            append(',')
            append("\"has_exported_key_bundle\":").append(hasExportedKeyBundle)
            append('}')
        }

        private fun escape(value: String): String =
            value.replace("\\", "\\\\").replace("\"", "\\\"")
    }

    private class Arguments(
        val queueFile: Path?,
        val jsonOutput: Boolean,
    ) {
        companion object {
            fun parse(args: Array<String>): Arguments {
                var file: Path? = null
                var json = false
                var i = 0
                while (i < args.size) {
                    when (args[i]) {
                        "--file" -> if (i + 1 < args.size) file = Path.of(args[++i])
                        "--json" -> json = true
                        else -> System.err.printf("Unknown argument: %s%n", args[i])
                    }
                    i++
                }
                return Arguments(file, json)
            }
        }
    }
}
