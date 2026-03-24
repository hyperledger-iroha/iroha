package org.hyperledger.iroha.sdk.offline

import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.core.util.HashLiteral
import java.io.IOException
import java.nio.file.Files
import java.nio.file.Path

/** Immutable representation of an `AndroidProvisionedProof` payload. */
class AndroidProvisionedProof private constructor(
    val manifestSchema: String,
    val manifestVersion: Int?,
    val manifestIssuedAtMs: Long,
    val challengeHashLiteral: String,
    val counter: Long,
    deviceManifest: Map<String, Any>,
    val inspectorSignatureHex: String,
) {
    private val _deviceManifest: Map<String, Any> = deviceManifest.toMap()

    val deviceManifest: Map<String, Any> get() = _deviceManifest

    val deviceId: String?
        get() = asString(_deviceManifest["android.provisioned.device_id"])

    fun inspectorSignature(): ByteArray = hexToBytes(inspectorSignatureHex)

    fun challengeHashBytes(): ByteArray = HashLiteral.decode(challengeHashLiteral)

    /** Returns a canonical JSON representation (keys sorted lexicographically). */
    fun toCanonicalJson(): String {
        val payload = LinkedHashMap<String, Any>()
        payload["manifest_schema"] = manifestSchema
        if (manifestVersion != null) {
            payload["manifest_version"] = manifestVersion
        }
        payload["manifest_issued_at_ms"] = manifestIssuedAtMs
        payload["challenge_hash"] = challengeHashLiteral
        payload["counter"] = counter
        payload["device_manifest"] = _deviceManifest
        payload["inspector_signature"] = inspectorSignatureHex
        return JsonEncoder.encode(payload)
    }

    companion object {
        @JvmStatic
        fun fromJson(payload: ByteArray): AndroidProvisionedProof {
            val root = parse(payload)
            return fromObject(root)
        }

        @JvmStatic
        fun fromJson(json: String): AndroidProvisionedProof {
            val root = JsonParser.parse(json.trim()) ?: throw IllegalStateException("JSON parsed to null")
            return fromObject(root)
        }

        @JvmStatic
        @Throws(IOException::class)
        fun fromPath(path: Path): AndroidProvisionedProof =
            fromJson(Files.readAllBytes(path))

        private fun fromObject(root: Any): AndroidProvisionedProof {
            val obj = expectObject(root, "root")
            val schema = requireString(obj["manifest_schema"], "manifest_schema")
            val manifestVersion = asOptionalInt(obj["manifest_version"])
            val issuedAt = requireLong(obj["manifest_issued_at_ms"], "manifest_issued_at_ms")
            val hashLiteral = requireString(obj["challenge_hash"], "challenge_hash")
            val counter = requireLong(obj["counter"], "counter")
            val manifest = expectObject(obj["device_manifest"], "device_manifest")
            val signature = requireString(obj["inspector_signature"], "inspector_signature")
            val canonicalSchema = schema.trim()
            check(canonicalSchema.isNotEmpty()) { "manifest_schema must not be empty" }
            val manifestCopy = copyManifest(manifest)
            val deviceId = asString(manifestCopy["android.provisioned.device_id"])
            check(!deviceId.isNullOrBlank()) { "device_manifest must include android.provisioned.device_id" }
            val canonicalHash = HashLiteral.canonicalize(hashLiteral)
            val canonicalSignature = normalizeSignature(signature)
            return AndroidProvisionedProof(
                canonicalSchema,
                manifestVersion,
                issuedAt,
                canonicalHash,
                counter,
                manifestCopy,
                canonicalSignature,
            )
        }

        private fun parse(payload: ByteArray): Any {
            val json = String(payload, Charsets.UTF_8).trim()
            check(json.isNotEmpty()) { "Empty JSON payload" }
            return JsonParser.parse(json) ?: throw IllegalStateException("JSON payload parsed to null")
        }

        @Suppress("UNCHECKED_CAST")
        private fun expectObject(value: Any?, path: String): Map<String, Any> {
            check(value is Map<*, *>) { "$path is not a JSON object" }
            return value as Map<String, Any>
        }

        private fun requireString(value: Any?, path: String): String {
            val normalized = asString(value)
            check(!normalized.isNullOrBlank()) { "$path is required" }
            return normalized
        }

        private fun asString(value: Any?): String? {
            if (value == null) return null
            if (value is String) return value
            return value.toString()
        }

        private fun requireLong(value: Any?, path: String): Long {
            check(value is Number) { "$path is not a number" }
            check(value !is Float && value !is Double) { "$path must be an integer" }
            return value.toLong()
        }

        private fun asOptionalInt(value: Any?): Int? {
            if (value == null) return null
            if (value is Number) {
                if (value is Float || value is Double) return null
                return value.toInt()
            }
            val normalized = asString(value)
            if (normalized.isNullOrBlank()) return null
            return normalized.toInt(10)
        }

        private fun copyManifest(manifest: Map<String, Any>): Map<String, Any> {
            val copy = LinkedHashMap<String, Any>()
            manifest.forEach { (key, value) ->
                copy[requireNotNull(key) { "manifest key" }] = copyValue(value) as Any
            }
            return copy.toMap()
        }

        @Suppress("UNCHECKED_CAST")
        private fun copyValue(value: Any?): Any? {
            if (value is Map<*, *>) {
                val nested = LinkedHashMap<String, Any>()
                value.forEach { (key, entry) -> nested[key.toString()] = copyValue(entry) as Any }
                return nested.toMap()
            }
            if (value is Iterable<*>) {
                return value.map { copyValue(it) }
            }
            if (value == null || value is String || value is Number || value is Boolean) {
                return value
            }
            return value.toString()
        }

        private fun normalizeSignature(value: String): String {
            val trimmed = value.trim()
            check(trimmed.matches(Regex("^[0-9A-Fa-f]{128}$"))) { "inspector_signature must be 64-byte hex" }
            return trimmed.uppercase()
        }

        private fun hexToBytes(hex: String): ByteArray {
            val bytes = ByteArray(hex.length / 2)
            for (i in bytes.indices) {
                val offset = i * 2
                bytes[i] = hex.substring(offset, offset + 2).toInt(16).toByte()
            }
            return bytes
        }
    }
}
