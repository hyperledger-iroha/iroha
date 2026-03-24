package org.hyperledger.iroha.sdk.client

import java.io.IOException
import java.net.URI
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Path
import java.security.MessageDigest
import java.time.Duration
import java.time.Instant
import java.util.Collections
import java.util.LinkedHashMap
import java.util.function.BiConsumer
import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi
import org.hyperledger.iroha.sdk.offline.OfflineJournalKey
import org.hyperledger.iroha.sdk.telemetry.Redaction
import org.hyperledger.iroha.sdk.telemetry.TelemetryOptions

/**
 * Loads [ClientConfig] instances from JSON manifests derived from `iroha_config`.
 *
 * The loader keeps the manifest structure available via [ManifestContext] so callers can
 * inspect the inputs alongside the built [ClientConfig].
 */
@OptIn(ExperimentalEncodingApi::class)
object ClientConfigManifestLoader {

    @JvmStatic @Throws(IOException::class) fun load(manifestPath: Path): LoadedClientConfig = load(manifestPath, null)

    @JvmStatic @Throws(IOException::class)
    fun load(manifestPath: Path, customizer: Customizer?): LoadedClientConfig {
        val payload = Files.readAllBytes(manifestPath)
        val digest = sha256Hex(payload)
        return parse(manifestPath, payload, digest, customizer)
    }

    @JvmStatic
    internal fun parse(manifestPath: Path, payload: ByteArray, digest: String, customizer: Customizer?): LoadedClientConfig {
        val root = parseRoot(payload)
        val builder = ClientConfig.builder()
        applyTorii(manifestPath, builder, root)
        applyRetry(builder, root)
        applyPendingQueue(manifestPath, builder, root)
        applyTelemetry(builder, root)
        val context = ManifestContext(manifestPath, digest, immutableCopy(root))
        customizer?.accept(builder, context)
        return LoadedClientConfig(builder.build(), context, Instant.now())
    }

    private fun parseRoot(payload: ByteArray): MutableMap<String, Any?> {
        val json = String(payload, StandardCharsets.UTF_8).trim()
        check(json.isNotEmpty()) { "Client config manifest is empty" }
        return expectObject(JsonParser.parse(json), "manifest")
    }

    private fun applyTorii(manifestPath: Path, builder: ClientConfig.Builder, root: Map<String, Any?>) {
        val torii = expectObject(root["torii"], "torii")
        builder.setBaseUri(parseUri(requireString(torii, "base_uri"), "torii.base_uri"))
        val gateway = optionalString(torii["sorafs_gateway_uri"])
        if (!gateway.isNullOrEmpty()) builder.setSorafsGatewayUri(parseUri(gateway, "torii.sorafs_gateway_uri"))
        val timeoutMs = optionalLong(torii["timeout_ms"])
        if (timeoutMs != null && timeoutMs >= 0) builder.setRequestTimeout(Duration.ofMillis(timeoutMs))
        val headers = optionalObject(torii["default_headers"], "torii.default_headers")
        headers?.forEach { (name, value) -> optionalString(value)?.let { builder.putDefaultHeader(name, it) } }
    }

    private fun applyRetry(builder: ClientConfig.Builder, root: Map<String, Any?>) {
        val retry = optionalObject(root["retry"], "retry") ?: return
        val policy = RetryPolicy.builder()
        optionalInt(retry["max_attempts"])?.takeIf { it >= 1 }?.let { policy.setMaxAttempts(it) }
        optionalLong(retry["base_delay_ms"])?.takeIf { it >= 0 }?.let { policy.setBaseDelay(Duration.ofMillis(it)) }
        optionalLong(retry["max_delay_ms"])?.takeIf { it >= 0 }?.let { policy.setMaxDelay(Duration.ofMillis(it)) }
        optionalBoolean(retry["retry_on_server_error"])?.let { policy.setRetryOnServerError(it) }
        optionalBoolean(retry["retry_on_too_many_requests"])?.let { policy.setRetryOnTooManyRequests(it) }
        optionalBoolean(retry["retry_on_network_error"])?.let { policy.setRetryOnNetworkError(it) }
        optionalArray(retry["retry_status_codes"], "retry.retry_status_codes")?.forEach { optionalInt(it)?.let { s -> policy.addRetryStatusCode(s) } }
        builder.setRetryPolicy(policy.build())
    }

    private fun applyPendingQueue(manifestPath: Path, builder: ClientConfig.Builder, root: Map<String, Any?>) {
        val pending = optionalObject(root["pending_queue"], "pending_queue") ?: return
        val kind = optionalString(pending["kind"])
        if (kind == null || "memory".equals(kind, ignoreCase = true)) { builder.setPendingQueue(null); return }
        if ("offline_journal".equals(kind, ignoreCase = true)) {
            val pathRaw = requireString(pending, "path")
            val resolved = resolveRelative(manifestPath, pathRaw)
            val key = deriveJournalKey(pending)
            check(key != null) { "pending_queue.kind=offline_journal requires either key_seed_b64 or passphrase" }
            builder.enableOfflineJournalQueue(resolved, key); return
        }
        if ("file".equals(kind, ignoreCase = true)) {
            val pathRaw = requireString(pending, "path")
            builder.enableFilePendingQueue(resolveRelative(manifestPath, pathRaw)); return
        }
        throw IllegalStateException("Unsupported pending queue kind: $kind")
    }

    private fun deriveJournalKey(pending: Map<String, Any?>): OfflineJournalKey? {
        val seedB64 = optionalString(pending["key_seed_b64"])
        if (!seedB64.isNullOrEmpty()) {
            val seed = Base64.decode(seedB64)
            check(seed.isNotEmpty()) { "pending_queue.key_seed_b64 cannot decode to an empty seed" }
            return OfflineJournalKey.derive(seed)
        }
        val seedHex = optionalString(pending["key_seed_hex"])
        if (!seedHex.isNullOrBlank()) return OfflineJournalKey.derive(hexToBytes(seedHex.trim()))
        val passphrase = optionalString(pending["passphrase"])
        if (!passphrase.isNullOrEmpty()) return OfflineJournalKey.deriveFromPassphrase(passphrase.toCharArray())
        return null
    }

    private fun applyTelemetry(builder: ClientConfig.Builder, root: Map<String, Any?>) {
        val telemetry = optionalObject(root["telemetry"], "telemetry")
        if (telemetry == null) { builder.setTelemetryOptions(TelemetryOptions.disabled()); return }
        val enabledFlag = optionalBoolean(telemetry["enabled"])
        val enabled = enabledFlag == null || enabledFlag
        val telemetryBuilder = TelemetryOptions.builder().setEnabled(enabled)
        if (enabled) {
            val redaction = optionalObject(telemetry["redaction"], "telemetry.redaction")
            check(redaction != null) { "telemetry.redaction block required when telemetry.enabled is true" }
            telemetryBuilder.setTelemetryRedaction(parseRedaction(redaction))
        } else { telemetryBuilder.setTelemetryRedaction(Redaction.disabled()) }
        builder.setTelemetryOptions(telemetryBuilder.build())
        optionalString(telemetry["exporter_name"])?.let { builder.setTelemetryExporterName(it) }
    }

    private fun parseRedaction(redaction: Map<String, Any?>): Redaction {
        val rb = Redaction.builder()
        val saltB64 = optionalString(redaction["salt_b64"])
        val saltHex = optionalString(redaction["salt_hex"])
        if (!saltB64.isNullOrEmpty()) rb.setSalt(Base64.decode(saltB64))
        else if (!saltHex.isNullOrBlank()) rb.setSaltHex(saltHex.trim())
        rb.setSaltVersion(requireString(redaction, "salt_version"))
        val rotation = optionalString(redaction["rotation_id"])
        rb.setRotationId(if (!rotation.isNullOrEmpty()) rotation else requireString(redaction, "salt_version"))
        optionalString(redaction["algorithm"])?.takeIf { it.isNotEmpty() }?.let { rb.setAlgorithm(it) }
        return rb.build()
    }

    private fun immutableCopy(input: Map<String, Any?>): Map<String, Any?> {
        val copy = LinkedHashMap<String, Any?>()
        for ((k, v) in input) copy[k] = immutableValue(v)
        return Collections.unmodifiableMap(copy)
    }

    private fun immutableValue(value: Any?): Any? = when (value) {
        is Map<*, *> -> Collections.unmodifiableMap(LinkedHashMap<String, Any?>().also { copy -> value.forEach { (k, v) -> if (k is String) copy[k] = immutableValue(v) } })
        is List<*> -> Collections.unmodifiableList(value.map { immutableValue(it) })
        else -> value
    }

    @Suppress("UNCHECKED_CAST")
    private fun expectObject(value: Any?, path: String): MutableMap<String, Any?> {
        check(value != null) { "$path block is required" }
        check(value is Map<*, *>) { "$path is not a JSON object" }
        val copy = LinkedHashMap<String, Any?>()
        for ((k, v) in value) { if (k is String) copy[k] = v }
        return copy
    }

    @Suppress("UNCHECKED_CAST")
    private fun optionalObject(value: Any?, path: String): MutableMap<String, Any?>? {
        if (value == null) return null
        check(value is Map<*, *>) { "$path is not a JSON object" }
        val copy = LinkedHashMap<String, Any?>()
        for ((k, v) in value) { if (k is String) copy[k] = v }
        return copy
    }

    private fun optionalArray(value: Any?, path: String): List<Any?>? {
        if (value == null) return null
        check(value is List<*>) { "$path is not an array" }
        return ArrayList(value)
    }

    private fun requireString(obj: Map<String, Any?>, key: String): String {
        val value = obj[key]
        check(value != null) { "Missing required field: $key" }
        val normalized = optionalString(value)
        check(!normalized.isNullOrEmpty()) { "Field $key must be a non-empty string" }
        return normalized
    }

    private fun optionalString(value: Any?): String? = when (value) { null -> null; is String -> value; else -> value.toString() }

    private fun optionalInt(value: Any?): Int? {
        if (value == null) return null
        if (value is Number) {
            check(value !is Float && value !is Double) { "Fractional numbers are not supported: $value" }
            val longValue = value.toLong()
            check(longValue in Int.MIN_VALUE.toLong()..Int.MAX_VALUE.toLong()) { "Integer value out of range: $value" }
            return longValue.toInt()
        }
        return optionalString(value)?.toInt()
    }

    private fun optionalLong(value: Any?): Long? {
        if (value == null) return null
        if (value is Number) { check(value !is Float && value !is Double) { "Fractional numbers are not supported: $value" }; return value.toLong() }
        val normalized = optionalString(value)
        return if (normalized.isNullOrEmpty()) null else normalized.toLong()
    }

    private fun optionalBoolean(value: Any?): Boolean? {
        if (value == null) return null
        if (value is Boolean) return value
        val lower = optionalString(value)?.trim()?.lowercase() ?: return null
        return when (lower) { "true", "1", "yes" -> true; "false", "0", "no" -> false; else -> throw IllegalStateException("Unsupported boolean value: $value") }
    }

    private fun parseUri(value: String, field: String): URI {
        try { return URI.create(value) } catch (ex: IllegalArgumentException) { throw IllegalStateException("Invalid URI for $field: $value", ex) }
    }

    private fun resolveRelative(manifestPath: Path, target: String): Path {
        val baseDir = manifestPath.toAbsolutePath().parent ?: return Path.of(target).toAbsolutePath()
        return baseDir.resolve(target).normalize()
    }

    private fun hexToBytes(hex: String): ByteArray {
        val normalized = hex.trim()
        check(normalized.length % 2 == 0) { "Hex value must have an even number of characters" }
        val result = ByteArray(normalized.length / 2)
        for (i in normalized.indices step 2) {
            val hi = Character.digit(normalized[i], 16)
            val lo = Character.digit(normalized[i + 1], 16)
            check(hi >= 0 && lo >= 0) { "Hex value contains invalid characters" }
            result[i / 2] = ((hi shl 4) or lo).toByte()
        }
        return result
    }

    @JvmStatic
    internal fun sha256Hex(payload: ByteArray): String {
        val hash = MessageDigest.getInstance("SHA-256").digest(payload)
        return hash.joinToString("") { String.format("%02x", it) }
    }

    /** Allows callers to customise the builder after the manifest has been applied. */
    fun interface Customizer : BiConsumer<ClientConfig.Builder, ManifestContext>

    /** Immutable manifest metadata passed to customisers and returned in [LoadedClientConfig]. */
    class ManifestContext internal constructor(
        private val manifestPath: Path,
        private val digest: String,
        private val manifest: Map<String, Any?>
    ) {
        fun manifestPath(): Path = manifestPath
        fun digest(): String = digest
        fun manifest(): Map<String, Any?> = manifest
    }

    /** Wraps the built [ClientConfig] together with provenance metadata. */
    class LoadedClientConfig internal constructor(
        private val clientConfig: ClientConfig,
        private val context: ManifestContext,
        private val loadedAt: Instant
    ) {
        fun clientConfig(): ClientConfig = clientConfig
        fun context(): ManifestContext = context
        fun loadedAt(): Instant = loadedAt
    }
}
