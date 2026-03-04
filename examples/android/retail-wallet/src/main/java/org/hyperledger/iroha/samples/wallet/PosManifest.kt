package org.hyperledger.iroha.samples.wallet

import android.content.Context
import java.io.BufferedReader
import java.io.InputStreamReader
import java.security.KeyFactory
import java.security.Signature
import java.security.spec.X509EncodedKeySpec
import java.time.Clock
import java.time.Duration
import java.time.Instant
import java.time.ZoneOffset
import java.time.format.DateTimeFormatter
import java.util.Locale
import java.util.Base64
import org.json.JSONArray
import org.json.JSONObject
import org.hyperledger.iroha.android.address.AccountAddress
import org.hyperledger.iroha.android.address.AccountAddressException

data class PosProvisionManifest(
    val manifestId: String,
    val sequence: Int,
    val publishedAtMs: Long,
    val validFromMs: Long,
    val validUntilMs: Long,
    val rotationHintMs: Long?,
    val operator: String,
    val backendRoots: List<PosBackendRoot>,
    val metadata: JSONObject?,
    val payloadBase64: String,
    val operatorSignature: String
)

data class PosBackendRoot(
    val label: String,
    val role: String,
    val publicKey: String,
    val validFromMs: Long,
    val validUntilMs: Long,
    val metadata: JSONObject?
)

data class ManifestStatus(
    val manifestId: String,
    val sequence: Int,
    val operator: String,
    val validWindowLabel: String,
    val rotationLabel: String,
    val dualStatusLabel: String,
    val dualStatusHealthy: Boolean,
    val warnings: List<String>,
    val backendRoots: List<BackendRootStatus>
) {
    companion object {
        fun from(
            manifest: PosProvisionManifest,
            clock: Clock = Clock.systemUTC()
        ): ManifestStatus {
            val formatter = DateTimeFormatter.ISO_INSTANT.withZone(ZoneOffset.UTC)
            val validLabel = formatter.format(Instant.ofEpochMilli(manifest.validFromMs)) +
                " – " + formatter.format(Instant.ofEpochMilli(manifest.validUntilMs))
            val rotationLabel = manifest.rotationHintMs?.let { hint ->
                formatter.format(Instant.ofEpochMilli(hint))
            } ?: "n/a"
            val now = clock.millis()
            val backendStatuses = manifest.backendRoots.map { root ->
                val active = now in root.validFromMs..root.validUntilMs
                val statusLabel = buildBackendStatusLabel(root, active, formatter, now)
                BackendRootStatus(
                    label = root.label,
                    role = root.role,
                    statusLabel = statusLabel,
                    active = active
                )
            }
            val dualStatus = buildDualStatus(manifest, backendStatuses, now)
            val warnings = buildWarnings(manifest, backendStatuses, dualStatus, now, formatter)
            return ManifestStatus(
                manifestId = manifest.manifestId,
                sequence = manifest.sequence,
                operator = manifest.operator,
                validWindowLabel = validLabel,
                rotationLabel = rotationLabel,
                dualStatusLabel = dualStatus.label,
                dualStatusHealthy = dualStatus.healthy,
                warnings = warnings,
                backendRoots = backendStatuses
            )
        }

        private fun buildBackendStatusLabel(
            root: PosBackendRoot,
            active: Boolean,
            formatter: DateTimeFormatter,
            nowMs: Long
        ): String {
            val expiresIn = root.validUntilMs - nowMs
            val expiresLabel = if (expiresIn > 0) {
                "expires ${formatDuration(expiresIn)}"
            } else {
                "expired ${formatter.format(Instant.ofEpochMilli(root.validUntilMs))}"
            }
            val validity = formatter.format(Instant.ofEpochMilli(root.validFromMs)) +
                " – " + formatter.format(Instant.ofEpochMilli(root.validUntilMs))
            val state = if (active) "active" else "inactive"
            return "$state · $validity ($expiresLabel)"
        }

        private fun buildDualStatus(
            manifest: PosProvisionManifest,
            roots: List<BackendRootStatus>,
            now: Long
        ): DualStatus {
            val roles = roots.filter { it.active }.map { it.role }.toSet()
            val required = setOf("offline_admission_signer", "offline_allowance_witness")
            val missing = required.filterNot { roles.contains(it) }
            return if (missing.isEmpty()) {
                DualStatus(true, "admission + witness present until ${formatDuration(manifest.validUntilMs - now)}")
            } else {
                DualStatus(false, "missing ${missing.joinToString(", ")}")
            }
        }

        private fun buildWarnings(
            manifest: PosProvisionManifest,
            roots: List<BackendRootStatus>,
            dualStatus: DualStatus,
            now: Long,
            formatter: DateTimeFormatter
        ): List<String> {
            val warnings = mutableListOf<String>()
            if (now < manifest.validFromMs) {
                warnings.add("manifest not active until ${formatter.format(Instant.ofEpochMilli(manifest.validFromMs))}")
            }
            val manifestExpiresIn = manifest.validUntilMs - now
            if (manifestExpiresIn <= MANIFEST_WARNING_WINDOW_MS) {
                warnings.add("manifest expires in ${formatDuration(manifestExpiresIn)}")
            }
            manifest.rotationHintMs?.let { hint ->
                if (now >= hint) {
                    warnings.add("rotation hint passed ${formatDuration(now - hint)} ago")
                } else if (hint - now <= ROTATION_WARNING_WINDOW_MS) {
                    warnings.add("rotation hint in ${formatDuration(hint - now)}")
                }
            }
            roots.filter { it.active.not() }.forEach { inactive ->
                warnings.add("${inactive.label} inactive")
            }
            if (!dualStatus.healthy) {
                warnings.add(dualStatus.label)
            }
            if (warnings.isEmpty()) {
                return emptyList()
            }
            return warnings
        }

        private fun formatDuration(durationMs: Long): String {
            if (durationMs <= 0) {
                return "0s"
            }
            val duration = Duration.ofMillis(durationMs)
            val days = duration.toDays()
            val hours = duration.minusDays(days).toHours()
            val minutes = duration.minusDays(days).minusHours(hours).toMinutes()
            val parts = mutableListOf<String>()
            if (days > 0) {
                parts.add("${days}d")
            }
            if (hours > 0) {
                parts.add("${hours}h")
            }
            if (minutes > 0) {
                parts.add("${minutes}m")
            }
            if (parts.isEmpty()) {
                parts.add("${duration.seconds}s")
            }
            return parts.joinToString(" ")
        }
    }
}

data class BackendRootStatus(
    val label: String,
    val role: String,
    val statusLabel: String,
    val active: Boolean
)

private data class DualStatus(val healthy: Boolean, val label: String)

object PosManifestLoader {
    private const val MANIFEST_ASSET = "pos_manifest.json"

    fun loadFromAssets(context: Context): PosProvisionManifest {
        context.assets.open(MANIFEST_ASSET).use { input ->
            val reader = BufferedReader(InputStreamReader(input))
            val raw = reader.readText()
            return parse(raw)
        }
    }

    fun parse(raw: String): PosProvisionManifest {
        val json = JSONObject(raw)
        val backendRoots = parseBackendRoots(json.optJSONArray("backend_roots"))
        val payloadBase64 =
            json.optString("payload_base64")
                .takeIf { it.isNotBlank() }
                ?: throw IllegalArgumentException("manifest missing payload_base64")
        val operatorSignature =
            json.optString("operator_signature")
                .takeIf { it.isNotBlank() }
                ?: throw IllegalArgumentException("manifest missing operator_signature")
        val operator = json.getString("operator")
        val payload = decodeBase64(payloadBase64)
        val signature = decodeHex(operatorSignature)
        val operatorKey = decodeOperatorPublicKey(operator)
        verifySignature(operatorKey, payload, signature)
        return PosProvisionManifest(
            manifestId = json.getString("manifest_id"),
            sequence = json.getInt("sequence"),
            publishedAtMs = json.getLong("published_at_ms"),
            validFromMs = json.getLong("valid_from_ms"),
            validUntilMs = json.getLong("valid_until_ms"),
            rotationHintMs = json.optLong("rotation_hint_ms").takeIf { json.has("rotation_hint_ms") },
            operator = operator,
            backendRoots = backendRoots,
            metadata = json.optJSONObject("metadata"),
            payloadBase64 = payloadBase64,
            operatorSignature = operatorSignature
        )
    }

    private fun parseBackendRoots(array: JSONArray?): List<PosBackendRoot> {
        if (array == null) return emptyList()
        val items = mutableListOf<PosBackendRoot>()
        for (index in 0 until array.length()) {
            val entry = array.getJSONObject(index)
            items.add(
                PosBackendRoot(
                    label = entry.getString("label"),
                    role = entry.getString("role").lowercase(Locale.US),
                    publicKey = entry.getString("public_key"),
                    validFromMs = entry.getLong("valid_from_ms"),
                    validUntilMs = entry.getLong("valid_until_ms"),
                    metadata = entry.optJSONObject("metadata")
                )
            )
        }
        return items
    }

    private fun decodeOperatorPublicKey(operatorId: String): ByteArray {
        val address = operatorId.substringBefore("@")
        if (address.lowercase(Locale.US).startsWith("ed01")) {
            val raw = decodeHex(address)
            require(raw.size > 3 && raw[0] == 0xED.toByte() && raw[1] == 0x01.toByte()) {
                "unsupported operator key encoding"
            }
            val declaredLen = raw[2].toInt() and 0xFF
            require(declaredLen == raw.size - 3) { "unexpected operator key length" }
            return raw.copyOfRange(3, raw.size)
        }
        try {
            val decoded = AccountAddress.fromIH58(address, null)
            return extractSingleSignatoryKey(decoded.canonicalBytes())
        } catch (ex: AccountAddressException) {
            throw IllegalArgumentException("failed to parse operator account", ex)
        }
    }

    private fun extractSingleSignatoryKey(canonical: ByteArray): ByteArray {
        if (canonical.size < 4) {
            throw IllegalArgumentException("invalid canonical address length")
        }
        var cursor = 0
        val header = canonical[cursor++].toInt() and 0xFF
        val extensionFlag = header and 0x01
        val classBits = (header shr 3) and 0x03
        require(extensionFlag == 0) { "address extension flag set" }
        require(classBits == 0 || classBits == 1) { "unknown address class" }
        cursor += when (val domainTag = canonical[cursor++].toInt() and 0xFF) {
            0x00 -> 0
            0x01 -> 12
            0x02 -> 4
            else -> throw IllegalArgumentException("unknown domain tag: $domainTag")
        }
        if (cursor >= canonical.size) {
            throw IllegalArgumentException("invalid canonical address length")
        }
        val controllerTag = canonical[cursor++].toInt() and 0xFF
        require(controllerTag == 0x00) { "unsupported controller tag $controllerTag" }
        if (cursor + 2 > canonical.size) {
            throw IllegalArgumentException("invalid canonical address length")
        }
        val curveId = canonical[cursor++].toInt() and 0xFF
        require(curveId == 0x01) { "unsupported signing algorithm $curveId" }
        val keyLen = canonical[cursor++].toInt() and 0xFF
        val end = cursor + keyLen
        if (end != canonical.size) {
            throw IllegalArgumentException("unexpected trailing bytes in address payload")
        }
        return canonical.copyOfRange(cursor, end)
    }

    private fun decodeBase64(value: String): ByteArray {
        return try {
            Base64.getDecoder().decode(value)
        } catch (ex: IllegalArgumentException) {
            throw IllegalArgumentException("invalid base64 payload", ex)
        }
    }

    private fun decodeHex(value: String): ByteArray {
        val normalized = value.trim()
        if (normalized.length % 2 != 0) {
            throw IllegalArgumentException("hex string must have even length")
        }
        val out = ByteArray(normalized.length / 2)
        var index = 0
        while (index < normalized.length) {
            val hi = Character.digit(normalized[index], 16)
            val lo = Character.digit(normalized[index + 1], 16)
            if (hi == -1 || lo == -1) {
                throw IllegalArgumentException("invalid hex string")
            }
            out[index / 2] = ((hi shl 4) or lo).toByte()
            index += 2
        }
        return out
    }

    private fun verifySignature(publicKey: ByteArray, payload: ByteArray, signature: ByteArray) {
        require(publicKey.size == 32) { "operator public key must be 32 bytes" }
        val encoded = encodeEd25519PublicKey(publicKey)
        try {
            val keyFactory = KeyFactory.getInstance("Ed25519")
            val publicSpec = X509EncodedKeySpec(encoded)
            val verifier = Signature.getInstance("Ed25519")
            verifier.initVerify(keyFactory.generatePublic(publicSpec))
            verifier.update(payload)
            if (!verifier.verify(signature)) {
                throw IllegalArgumentException("manifest signature verification failed")
            }
        } catch (ex: Exception) {
            if (ex is IllegalArgumentException) throw ex
            throw IllegalArgumentException("manifest signature verification failed", ex)
        }
    }

    private fun encodeEd25519PublicKey(raw: ByteArray): ByteArray {
        // RFC 8410 SubjectPublicKeyInfo prefix for Ed25519
        val prefix =
            byteArrayOf(
                0x30, 0x2A,
                0x30, 0x05,
                0x06, 0x03,
                0x2B, 0x65, 0x70,
                0x03, 0x21, 0x00
            )
        return ByteArray(prefix.size + raw.size).also {
            System.arraycopy(prefix, 0, it, 0, prefix.size)
            System.arraycopy(raw, 0, it, prefix.size, raw.size)
        }
    }
}

private const val MANIFEST_WARNING_WINDOW_MS = 7L * 24 * 60 * 60 * 1000 // 7 days
private const val ROTATION_WARNING_WINDOW_MS = 3L * 24 * 60 * 60 * 1000 // 3 days
