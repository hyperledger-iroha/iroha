@file:OptIn(ExperimentalEncodingApi::class)

package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.core.util.HashLiteral
import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi

private const val DOMAIN_KEY = "domain_id"
private const val CALL_NAME_KEY = "call_name"

/** Shared helpers for flattening Kaigi instruction payloads to argument maps. */
object KaigiInstructionUtils {

    fun requireAction(arguments: Map<String, String>, expected: String) {
        val actual = require(arguments, "action")
        require(actual == expected) {
            "Instruction action must be '$expected', got '$actual'"
        }
    }

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
            // Long is the JVM carrier for Rust u64 fields. Values in the upper half of the
            // unsigned range intentionally have a negative signed representation.
            val parsed = java.lang.Long.parseUnsignedLong(value)
            require(java.lang.Long.toUnsignedString(parsed) == value) {
                "$fieldName must use canonical unsigned decimal syntax"
            }
            return parsed
        } catch (ex: NumberFormatException) {
            throw IllegalArgumentException("$fieldName must be an unsigned integer", ex)
        }
    }

    fun parseOptionalUnsignedLong(value: String?, fieldName: String): Long? {
        if (value == null) return null
        return parseUnsignedLong(value, fieldName)
    }

    fun parsePositiveInt(value: String, fieldName: String): Int {
        requireNotNull(value) { fieldName }
        try {
            val parsed = Integer.parseUnsignedInt(value)
            // Int is the JVM carrier for Rust u32 fields, so only the all-zero bit pattern is
            // invalid here. High-bit values are valid positive u32 values.
            if (parsed == 0) {
                throw IllegalArgumentException("$fieldName must be greater than zero")
            }
            require(Integer.toUnsignedString(parsed) == value) {
                "$fieldName must use canonical unsigned decimal syntax"
            }
            return parsed
        } catch (ex: NumberFormatException) {
            throw IllegalArgumentException("$fieldName must be a positive integer", ex)
        }
    }

    fun parseOptionalPositiveInt(value: String?, fieldName: String): Int? {
        if (value == null) return null
        return parsePositiveInt(value, fieldName)
    }

    fun parseNonNegativeInt(value: String, fieldName: String): Int {
        requireNotNull(value) { fieldName }
        try {
            val parsed = Integer.parseUnsignedInt(value)
            require(Integer.toUnsignedString(parsed) == value) {
                "$fieldName must use canonical unsigned decimal syntax"
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

    fun parseRoomPolicy(arguments: Map<String, String>, prefix: String): RoomPolicy {
        val policyKey = prefixKey(prefix, "policy")
        val policy = arguments.getOrDefault(policyKey, "Authenticated")
        val state = arguments[prefixKey(prefix, "state")]
        return RoomPolicy(policy, state)
    }

    fun appendRoomPolicy(
        roomPolicy: RoomPolicy,
        target: MutableMap<String, String>,
        prefix: String,
    ) {
        target[prefixKey(prefix, "policy")] = roomPolicy.policy
        if (roomPolicy.state != null) {
            target[prefixKey(prefix, "state")] = roomPolicy.state
        }
    }

    fun parseRelayManifest(arguments: Map<String, String>, prefix: String): RelayManifest? {
        val expiresKey = prefixKey(prefix, "expiry_ms")
        val expiryMs = parseOptionalUnsignedLong(arguments[expiresKey], expiresKey)

        val hopPrefix = prefixKey(prefix, "hop.")
        val hopArgumentCount = arguments.keys.count { it.startsWith(hopPrefix) }
        val hopsByIndex = sortedMapOf<Int, RelayManifestHop>()
        for ((key, value) in arguments) {
            if (!key.startsWith(hopPrefix)) continue
            val tail = key.substring(hopPrefix.length)
            val separator = tail.indexOf('.')
            if (separator <= 0) {
                throw IllegalArgumentException("Malformed relay manifest key: $key")
            }
            val index = try {
                tail.substring(0, separator).toInt()
            } catch (ex: NumberFormatException) {
                throw IllegalArgumentException("Relay manifest hop index must be numeric: $key", ex)
            }
            if (index.toString() != tail.substring(0, separator)) {
                throw IllegalArgumentException("Relay manifest hop index must be canonical: $key")
            }
            if (index !in 0 until hopArgumentCount) {
                throw IllegalArgumentException("Relay manifest hop index is out of bounds: $key")
            }
            val hop = hopsByIndex.getOrPut(index) { RelayManifestHop(null, null, null) }
            when (val attribute = tail.substring(separator + 1)) {
                "relay_id" -> hopsByIndex[index] = hop.copy(relayId = value)
                "hpke_public_key" -> {
                    hopsByIndex[index] = hop.copy(hpkePublicKey = requireBase64(value, key))
                }
                "weight" -> {
                    val parsed = parseNonNegativeInt(value, "relay hop weight")
                    if (parsed !in 1..0xFF) {
                        throw IllegalArgumentException("relay hop weight must be between 1 and 255")
                    }
                    hopsByIndex[index] = hop.copy(weight = parsed)
                }
                else -> throw IllegalArgumentException("Unknown relay manifest attribute: $key")
            }
        }
        if (hopsByIndex.isEmpty() && expiryMs == null) return null
        for ((expectedIndex, actualIndex) in hopsByIndex.keys.withIndex()) {
            if (actualIndex != expectedIndex) {
                throw IllegalArgumentException("relay manifest hop indices must be contiguous from zero")
            }
        }
        val hops = hopsByIndex.values.toList()
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
        return validateRelayManifest(RelayManifest(expiryMs, hops.toList()))
    }

    fun appendRelayManifest(
        manifest: RelayManifest?,
        target: MutableMap<String, String>,
        prefix: String,
    ) {
        if (manifest == null) return
        validateRelayManifest(manifest)
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

    fun validateRelayManifest(manifest: RelayManifest): RelayManifest {
        manifest.expiryMs
            ?: throw IllegalArgumentException("relay manifest expiry_ms is required")
        require(manifest.hops.size >= 3) { "relay manifest must contain at least 3 hops" }
        val relayIds = mutableSetOf<String>()
        for ((index, hop) in manifest.hops.withIndex()) {
            val relayId = hop.relayId
            if (relayId.isNullOrBlank()) {
                throw IllegalArgumentException("relay_manifest.hop.$index.relay_id is required")
            }
            if (!relayIds.add(relayId)) {
                throw IllegalArgumentException("relay manifest relay IDs must be unique")
            }
            requireBase64(hop.hpkePublicKey, "relay_manifest.hop.$index.hpke_public_key")
            val weight = hop.weight
                ?: throw IllegalArgumentException("relay_manifest.hop.$index.weight is required")
            require(weight in 1..0xFF) { "relay hop weight must be between 1 and 255" }
        }
        return manifest
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
    ) {
        init {
            require(domainId.isNotBlank()) { "Kaigi call domainId must not be blank" }
            require(callName.isNotBlank()) { "Kaigi callName must not be blank" }
        }
    }

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
            require(state == null) { "Kaigi privacy mode variants do not accept state" }
            this.mode = normalized
            this.state = null
        }
    }

    /** Immutable room access policy descriptor. */
    class RoomPolicy(policy: String, state: String?) {

        @JvmField val policy: String
        @JvmField val state: String?

        init {
            if (policy.isBlank()) {
                throw IllegalArgumentException("room policy must not be blank")
            }
            val normalized = policy.trim()
            if (normalized != "Public" && normalized != "Authenticated") {
                throw IllegalArgumentException("room policy must be Public or Authenticated")
            }
            require(state == null) { "Kaigi room policy variants do not accept state" }
            this.policy = normalized
            this.state = null
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
