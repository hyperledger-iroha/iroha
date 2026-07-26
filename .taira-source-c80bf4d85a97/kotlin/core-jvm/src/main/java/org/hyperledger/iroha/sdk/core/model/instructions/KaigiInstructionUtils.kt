@file:OptIn(ExperimentalEncodingApi::class)

package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.core.util.HashLiteral
import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi

private const val DOMAIN_KEY = "domain_id"
private const val CALL_NAME_KEY = "call_name"

/** Shared helpers for flattening Kaigi instruction payloads to argument maps. */
object KaigiInstructionUtils {

    fun parseCallId(arguments: Map<String, String>, prefix: String): CallId {
        val domain = require(arguments, prefixKey(prefix, DOMAIN_KEY))
        val callName = require(arguments, prefixKey(prefix, CALL_NAME_KEY))
        return CallId(domain, callName)
    }

    fun appendCallId(callId: CallId, target: MutableMap<String, String>, prefix: String) {
        target[prefixKey(prefix, DOMAIN_KEY)] = callId.domainId
        target[prefixKey(prefix, CALL_NAME_KEY)] = callId.callName
    }

    fun extractMetadata(arguments: Map<String, String>, prefix: String): Map<String, String> {
        val metadata = linkedMapOf<String, String>()
        val effectivePrefix = if (prefix.endsWith(".")) prefix else "$prefix."
        for ((key, value) in arguments) {
            if (key.startsWith(effectivePrefix)) {
                val metadataKey = key.substring(effectivePrefix.length)
                if (metadataKey.isNotEmpty()) {
                    metadata[metadataKey] = value
                }
            }
        }
        return metadata
    }

    fun appendMetadata(
        metadata: Map<String, String>,
        target: MutableMap<String, String>,
        prefix: String,
    ) {
        if (metadata.isEmpty()) return
        val effectivePrefix = if (prefix.endsWith(".")) prefix else "$prefix."
        for ((key, value) in metadata) {
            target["$effectivePrefix$key"] = value
        }
    }

    fun canonicalizeHash(bytes: ByteArray): String = HashLiteral.canonicalize(bytes)

    fun canonicalizeHash(value: String): String = HashLiteral.canonicalize(value)

    fun canonicalizeOptionalHash(value: String?): String? {
        if (value.isNullOrBlank()) return null
        return HashLiteral.canonicalizeOptional(value)
    }

    fun canonicalizeOptionalHash(value: ByteArray?): String? {
        if (value == null) return null
        return canonicalizeHash(value)
    }

    fun parseUnsignedLong(value: String, fieldName: String): Long {
        requireNotNull(value) { fieldName }
        try {
            val parsed = java.lang.Long.parseUnsignedLong(value)
            if (parsed < 0) {
                throw IllegalArgumentException("$fieldName must be unsigned")
            }
            return parsed
        } catch (ex: NumberFormatException) {
            throw IllegalArgumentException("$fieldName must be an unsigned integer", ex)
        }
    }

    fun parseOptionalUnsignedLong(value: String?, fieldName: String): Long? {
        if (value.isNullOrBlank()) return null
        return parseUnsignedLong(value, fieldName)
    }

    fun parsePositiveInt(value: String, fieldName: String): Int {
        requireNotNull(value) { fieldName }
        try {
            val parsed = Integer.parseUnsignedInt(value)
            if (parsed <= 0) {
                throw IllegalArgumentException("$fieldName must be greater than zero")
            }
            return parsed
        } catch (ex: NumberFormatException) {
            throw IllegalArgumentException("$fieldName must be a positive integer", ex)
        }
    }

    fun parseOptionalPositiveInt(value: String?, fieldName: String): Int? {
        if (value.isNullOrBlank()) return null
        return parsePositiveInt(value, fieldName)
    }

    fun parseNonNegativeInt(value: String, fieldName: String): Int {
        requireNotNull(value) { fieldName }
        try {
            val parsed = Integer.parseUnsignedInt(value)
            if (parsed < 0) {
                throw IllegalArgumentException("$fieldName must be non-negative")
            }
            return parsed
        } catch (ex: NumberFormatException) {
            throw IllegalArgumentException("$fieldName must be a non-negative integer", ex)
        }
    }

    fun toBase64(bytes: ByteArray): String {
        requireNotNull(bytes) { "bytes" }
        return Base64.encode(bytes)
    }

    fun requireBase64(value: String?, fieldName: String): String {
        if (value.isNullOrBlank()) {
            throw IllegalArgumentException("$fieldName must not be blank")
        }
        val trimmed = value.trim()
        val decoded: ByteArray
        try {
            decoded = Base64.decode(trimmed)
        } catch (ex: IllegalArgumentException) {
            throw IllegalArgumentException("$fieldName must be base64", ex)
        }
        if (decoded.isEmpty()) {
            throw IllegalArgumentException("$fieldName must decode to non-empty bytes")
        }
        return trimmed
    }

    fun parsePrivacyMode(arguments: Map<String, String>, prefix: String): PrivacyMode {
        val modeKey = prefixKey(prefix, "mode")
        val mode = arguments.getOrDefault(modeKey, "Transparent")
        val state = arguments[prefixKey(prefix, "state")]
        return PrivacyMode(mode, state)
    }

    fun appendPrivacyMode(
        privacyMode: PrivacyMode,
        target: MutableMap<String, String>,
        prefix: String,
    ) {
        target[prefixKey(prefix, "mode")] = privacyMode.mode
        if (privacyMode.state != null) {
            target[prefixKey(prefix, "state")] = privacyMode.state
        }
    }

    fun parseRelayManifest(arguments: Map<String, String>, prefix: String): RelayManifest? {
        val expiresKey = prefixKey(prefix, "expiry_ms")
        val expiryMs = parseOptionalUnsignedLong(arguments[expiresKey], expiresKey)

        val hops = mutableListOf<RelayManifestHop>()
        val hopPrefix = prefixKey(prefix, "hop.")
        for ((key, value) in arguments) {
            if (!key.startsWith(hopPrefix)) continue
            val tail = key.substring(hopPrefix.length)
            val separator = tail.indexOf('.')
            if (separator <= 0) {
                throw IllegalArgumentException("Malformed relay manifest key: $key")
            }
            val index = tail.substring(0, separator).toInt()
            while (hops.size <= index) {
                hops.add(RelayManifestHop(null, null, null))
            }
            val hop = hops[index]
            when (val attribute = tail.substring(separator + 1)) {
                "relay_id" -> hops[index] = hop.copy(relayId = value)
                "hpke_public_key" -> hops[index] = hop.copy(hpkePublicKey = requireBase64(value, key))
                "weight" -> {
                    val parsed = parseNonNegativeInt(value, "relay hop weight")
                    if (parsed > 0xFF) {
                        throw IllegalArgumentException("relay hop weight must fit in a byte")
                    }
                    hops[index] = hop.copy(weight = parsed)
                }
                else -> throw IllegalArgumentException("Unknown relay manifest attribute: $key")
            }
        }
        if (hops.isEmpty() && expiryMs == null) return null
        for (index in hops.indices) {
            val hop = hops[index]
            if (hop.relayId.isNullOrBlank()) {
                throw IllegalArgumentException("relay_manifest.hop.$index.relay_id is required")
            }
            if (hop.hpkePublicKey.isNullOrBlank()) {
                throw IllegalArgumentException("relay_manifest.hop.$index.hpke_public_key is required")
            }
            if (hop.weight == null) {
                throw IllegalArgumentException("relay_manifest.hop.$index.weight is required")
            }
        }
        return RelayManifest(expiryMs, hops.toList())
    }

    fun appendRelayManifest(
        manifest: RelayManifest?,
        target: MutableMap<String, String>,
        prefix: String,
    ) {
        if (manifest == null) return
        if (manifest.expiryMs != null) {
            target[prefixKey(prefix, "expiry_ms")] = java.lang.Long.toUnsignedString(manifest.expiryMs)
        }
        for (index in manifest.hops.indices) {
            val hop = manifest.hops[index]
            val baseKey = prefixKey(prefix, "hop.$index")
            target["$baseKey.relay_id"] = hop.relayId!!
            target["$baseKey.hpke_public_key"] = hop.hpkePublicKey!!
            target["$baseKey.weight"] = Integer.toUnsignedString(hop.weight!!)
        }
    }

    fun prefixKey(prefix: String?, key: String): String {
        if (prefix.isNullOrEmpty()) return key
        return if (prefix.endsWith(".")) "$prefix$key" else "$prefix.$key"
    }

    fun require(arguments: Map<String, String>, key: String): String {
        val value = arguments[key]
        if (value.isNullOrBlank()) {
            throw IllegalArgumentException("Instruction argument '$key' is required")
        }
        return value
    }

    /** Immutable representation of a Kaigi call identifier. */
    data class CallId(
        @JvmField val domainId: String,
        @JvmField val callName: String,
    )

    /** Immutable privacy configuration descriptor. */
    class PrivacyMode(mode: String, state: String?) {

        @JvmField val mode: String
        @JvmField val state: String?

        init {
            if (mode.isBlank()) {
                throw IllegalArgumentException("privacy mode must not be blank")
            }
            val normalized = mode.trim()
            if (normalized != "Transparent" && normalized != "ZkRosterV1") {
                throw IllegalArgumentException("privacy mode must be Transparent or ZkRosterV1")
            }
            this.mode = normalized
            this.state = if (state.isNullOrBlank()) null else state
        }
    }

    /** Immutable relay manifest snapshot. */
    data class RelayManifest(
        @JvmField val expiryMs: Long?,
        @JvmField val hops: List<RelayManifestHop>,
    )

    /** Single relay hop entry. */
    data class RelayManifestHop(
        @JvmField val relayId: String?,
        @JvmField val hpkePublicKey: String?,
        @JvmField val weight: Int?,
    )
}
