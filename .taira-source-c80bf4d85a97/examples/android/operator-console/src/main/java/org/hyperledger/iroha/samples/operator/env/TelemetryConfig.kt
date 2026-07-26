package org.hyperledger.iroha.samples.operator.env

import java.io.IOException
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.nio.file.StandardOpenOption
import java.time.Clock
import java.time.Instant
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock
import org.hyperledger.iroha.android.client.ClientObserver
import org.hyperledger.iroha.android.client.ClientResponse
import org.hyperledger.iroha.android.telemetry.TelemetryExportStatusSink
import org.hyperledger.iroha.android.telemetry.TelemetryObserver
import org.hyperledger.iroha.android.telemetry.TelemetryOptions
import org.hyperledger.iroha.android.telemetry.TelemetryRecord
import org.hyperledger.iroha.android.telemetry.TelemetrySink

data class TelemetryConfig(
    val enabled: Boolean,
    val logPath: String?,
    val saltHex: String,
    val saltVersion: String,
    val rotationId: String,
    val exporter: String
) {

    fun toTelemetryOptions(): TelemetryOptions {
        if (!enabled) {
            return TelemetryOptions.disabled()
        }
        return try {
            val redaction =
                TelemetryOptions.Redaction.builder()
                    .setEnabled(true)
                    .setSaltHex(saltHex)
                    .setSaltVersion(saltVersion)
                    .setRotationId(rotationId)
                    .build()
            TelemetryOptions.builder().setTelemetryRedaction(redaction).build()
        } catch (_: Exception) {
            TelemetryOptions.disabled()
        }
    }

    fun buildObserver(clock: Clock = Clock.systemUTC()): ClientObserver? {
        if (!enabled) {
            return null
        }
        val options = toTelemetryOptions()
        if (!options.enabled()) {
            return null
        }
        val baseSink = TelemetryLogSink(logPath, exporter, rotationId, clock)
        val sink = TelemetryExportStatusSink.wrap(baseSink, exporter)
        return TelemetryObserver(options, sink)
    }

    companion object {
        const val DEFAULT_SALT_HEX =
            "9f14c3d5d6e7f809aabbccddeeff11223344556677889900aabbccddeeff0011"
        const val DEFAULT_SALT_VERSION = "2027Q2-sample"
        const val DEFAULT_ROTATION_ID = "operator-sample"
        const val DEFAULT_EXPORTER = "operator-console"

        fun fromEnvironment(environment: SampleEnvironment): TelemetryConfig =
            TelemetryConfig(
                enabled = !environment.telemetryLogPath.isNullOrBlank(),
                logPath = environment.telemetryLogPath,
                saltHex = environment.telemetrySaltHex?.takeIf { it.isNotBlank() } ?: DEFAULT_SALT_HEX,
                saltVersion =
                    environment.telemetrySaltVersion?.takeIf { it.isNotBlank() }
                        ?: DEFAULT_SALT_VERSION,
                rotationId =
                    environment.telemetryRotationId?.takeIf { it.isNotBlank() }
                        ?: DEFAULT_ROTATION_ID,
                exporter =
                    environment.telemetryExporter?.takeIf { it.isNotBlank() }
                        ?: environment.profile.ifBlank { DEFAULT_EXPORTER }
            )
    }
}

class TelemetryLogSink(
    logPath: String?,
    private val exporter: String,
    private val rotationId: String,
    private val clock: Clock = Clock.systemUTC()
) : TelemetrySink {

    private val destination: Path? =
        logPath?.trim()?.takeIf { it.isNotEmpty() }?.let { path ->
            runCatching { Paths.get(path) }.getOrNull()
        }
    private val lock = ReentrantLock()

    override fun onRequest(record: TelemetryRecord) {
        writeEvent("request", record, emptyMap())
    }

    override fun onResponse(record: TelemetryRecord, response: ClientResponse) {
        writeEvent(
            "response",
            record,
            mapOf(
                "status_code" to response.statusCode(),
                "body_size" to response.body().size
            )
        )
    }

    override fun onFailure(record: TelemetryRecord, error: Throwable) {
        writeEvent(
            "failure",
            record,
            mapOf("error_message" to (error.message ?: error.javaClass.simpleName))
        )
    }

    override fun emitSignal(signalId: String, fields: Map<String, Any>) {
        val base =
            linkedMapOf<String, Any?>(
                "timestamp" to timestamp(),
                "exporter" to exporter,
                "event" to "signal",
                "signal_id" to signalId
            )
        for ((key, value) in fields) {
            base[key] = value
        }
        write(base)
    }

    private fun writeEvent(
        event: String,
        record: TelemetryRecord,
        extras: Map<String, Any?>
    ) {
        val fields =
            linkedMapOf<String, Any?>(
                "timestamp" to timestamp(),
                "exporter" to exporter,
                "event" to event,
                "salt_version" to record.saltVersion(),
                "rotation_id" to rotationId,
                "authority_hash" to record.authorityHash(),
                "route" to record.route(),
                "method" to record.method(),
                "latency_ms" to record.latencyOrNull(),
                "status_code" to record.statusCodeOrNull(),
                "error_kind" to record.errorKind().orElse(null)
            )
        extras.forEach { (key, value) -> fields[key] = value }
        write(fields)
    }

    private fun write(fields: Map<String, Any?>) {
        val target = destination ?: return
        val line = encodeJson(fields)
        lock.withLock {
            try {
                target.parent?.let { Files.createDirectories(it) }
                Files.writeString(
                    target,
                    "$line\n",
                    StandardOpenOption.CREATE,
                    StandardOpenOption.APPEND
                )
            } catch (_: IOException) {
                // Best-effort telemetry; drop records if the path is unavailable.
            }
        }
    }

    private fun timestamp(): String = Instant.now(clock).toString()

    private fun encodeJson(fields: Map<String, Any?>): String {
        val builder = StringBuilder()
        builder.append('{')
        fields.entries.forEachIndexed { index, entry ->
            if (index > 0) builder.append(',')
            builder.append('"').append(escape(entry.key)).append('"').append(':')
            builder.append(formatValue(entry.value))
        }
        builder.append('}')
        return builder.toString()
    }

    private fun formatValue(value: Any?): String =
        when (value) {
            null -> "null"
            is Number, is Boolean -> value.toString()
            else -> "\"${escape(value.toString())}\""
        }

    private fun escape(raw: String): String =
        raw.replace("\\", "\\\\").replace("\"", "\\\"").replace("\n", "\\n").replace("\r", "\\r")
}

private fun TelemetryRecord.latencyOrNull(): Long? {
    val opt = latencyMillis()
    return if (opt.isPresent) opt.asLong else null
}

private fun TelemetryRecord.statusCodeOrNull(): Int? {
    val opt = statusCode()
    return if (opt.isPresent) opt.asInt else null
}
