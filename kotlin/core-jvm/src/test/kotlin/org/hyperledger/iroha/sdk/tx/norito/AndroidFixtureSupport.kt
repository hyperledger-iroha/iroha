package org.hyperledger.iroha.sdk.tx.norito

import java.nio.file.Files
import java.nio.file.Path
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.core.model.TransactionPayload

internal data class TransactionPayloadFixture(
    val name: String,
    val chain: String,
    val authority: String,
    val creationTimeMs: Long,
    val timeToLiveMs: Long?,
    val nonce: Int?,
    val payload: Map<String, Any?>?,
    val encodedBase64: String?,
) {
    fun materializePayload(adapter: NoritoJavaCodecAdapter): TransactionPayload {
        payload?.let { return AndroidFixtureSupport.buildPayload(name, it) }
        check(!encodedBase64.isNullOrBlank()) { "$name: fixture missing payload and encoded data" }
        return adapter.decodeTransaction(java.util.Base64.getDecoder().decode(encodedBase64))
    }
}

internal data class TransactionManifestFixture(
    val name: String,
    val chain: String,
    val authority: String,
    val creationTimeMs: Long,
    val timeToLiveMs: Long?,
    val nonce: Long?,
    val payloadBase64: String,
    val payloadHash: String,
    val encodedFile: String,
    val encodedLen: Long,
    val signedBase64: String,
    val signedHash: String,
    val signedLen: Long,
)

internal object AndroidFixtureSupport {
    private const val ANDROID_RESOURCE_ROOT = "java/iroha_android/src/test/resources"

    fun loadPayloadFixtures(): List<TransactionPayloadFixture> {
        val path = resolveSharedResource("transaction_payloads.json")
        val parsed = JsonParser.parse(Files.readString(path))
        return asList(parsed, path.toString()).map { entry ->
            payloadFixtureFromValue(entry)
        }
    }

    fun payloadFixtureFromValue(value: Any?): TransactionPayloadFixture {
        val map = asMap(value, "payload fixture")
        val name = requiredString(map["name"], "payload fixture.name")
        val chain = requiredString(map["chain"], "$name.chain")
        val authority = requiredString(map["authority"], "$name.authority")
        val creationTimeMs = requiredLong(map["creation_time_ms"], "$name.creation_time_ms")
        val timeToLiveMs = optionalLong(map["time_to_live_ms"], "$name.time_to_live_ms")
        val nonce = optionalInt(map["nonce"], "$name.nonce")
        val payload = map["payload"]?.let { asMap(it, "$name.payload") }
        val encodedBase64 = optionalString(map["encoded"]) ?: optionalString(map["payload_base64"])
        return TransactionPayloadFixture(
            name = name,
            chain = chain,
            authority = authority,
            creationTimeMs = creationTimeMs,
            timeToLiveMs = timeToLiveMs,
            nonce = nonce,
            payload = payload,
            encodedBase64 = encodedBase64,
        )
    }

    fun loadManifestFixtures(): List<TransactionManifestFixture> {
        val path = resolveSharedResource("transaction_fixtures.manifest.json")
        val parsed = JsonParser.parse(Files.readString(path))
        val manifest = asMap(parsed, path.toString())
        return asList(manifest["fixtures"], "manifest.fixtures").map { entry ->
            val map = asMap(entry, "manifest.fixture")
            val name = requiredString(map["name"], "manifest.fixture.name")
            TransactionManifestFixture(
                name = name,
                chain = requiredString(map["chain"], "$name.chain"),
                authority = requiredString(map["authority"], "$name.authority"),
                creationTimeMs = requiredLong(map["creation_time_ms"], "$name.creation_time_ms"),
                timeToLiveMs = optionalLong(map["time_to_live_ms"], "$name.time_to_live_ms"),
                nonce = optionalLong(map["nonce"], "$name.nonce"),
                payloadBase64 = requiredString(map["payload_base64"], "$name.payload_base64"),
                payloadHash = requiredString(map["payload_hash"], "$name.payload_hash"),
                encodedFile = requiredString(map["encoded_file"], "$name.encoded_file"),
                encodedLen = requiredLong(map["encoded_len"], "$name.encoded_len"),
                signedBase64 = requiredString(map["signed_base64"], "$name.signed_base64"),
                signedHash = requiredString(map["signed_hash"], "$name.signed_hash"),
                signedLen = requiredLong(map["signed_len"], "$name.signed_len"),
            )
        }
    }

    fun resolveSharedResource(name: String): Path {
        var current = Path.of("").toAbsolutePath().normalize()
        while (true) {
            val candidate = current.resolve(ANDROID_RESOURCE_ROOT).resolve(name)
            if (Files.exists(candidate)) {
                return candidate
            }
            val parent = current.parent ?: break
            current = parent
        }
        error("Unable to locate $ANDROID_RESOURCE_ROOT/$name from ${Path.of("").toAbsolutePath()}")
    }

    internal fun buildPayload(name: String, payload: Map<String, Any?>): TransactionPayload {
        val executableMap = asMap(payload["executable"], "$name.payload.executable")
        val executable = when {
            executableMap.containsKey("Ivm") -> {
                val bytes = java.util.Base64.getDecoder().decode(
                    requiredString(executableMap["Ivm"], "$name.payload.executable.Ivm"),
                )
                Executable.ivm(bytes)
            }

            executableMap.containsKey("Instructions") -> {
                val instructions = asList(
                    executableMap["Instructions"],
                    "$name.payload.executable.Instructions",
                ).mapIndexed { index, raw ->
                    val instruction = asMap(raw, "$name.payload.executable.Instructions[$index]")
                    require(instruction.size == 2) {
                        "$name: instruction entries must only include wire_name and payload_base64"
                    }
                    val wireName = requiredString(
                        instruction["wire_name"],
                        "$name.payload.executable.Instructions[$index].wire_name",
                    )
                    val payloadBase64 = requiredString(
                        instruction["payload_base64"],
                        "$name.payload.executable.Instructions[$index].payload_base64",
                    )
                    val bytes = try {
                        java.util.Base64.getDecoder().decode(payloadBase64)
                    } catch (ex: IllegalArgumentException) {
                        throw IllegalStateException(
                            "$name: instruction payload_base64 is not valid base64",
                            ex,
                        )
                    }
                    InstructionBox.fromWirePayload(wireName, bytes)
                }
                Executable.instructions(instructions)
            }

            else -> error("$name: executable variant missing")
        }

        val metadata = payload["metadata"]?.let { raw ->
            asMap(raw, "$name.payload.metadata").mapValues { (_, value) ->
                value?.toString() ?: "null"
            }
        } ?: emptyMap()

        return TransactionPayload(
            chainId = requiredString(payload["chain"], "$name.payload.chain"),
            authority = requiredString(payload["authority"], "$name.payload.authority"),
            creationTimeMs = requiredLong(payload["creation_time_ms"], "$name.payload.creation_time_ms"),
            executable = executable,
            timeToLiveMs = optionalLong(payload["time_to_live_ms"], "$name.payload.time_to_live_ms"),
            nonce = optionalInt(payload["nonce"], "$name.payload.nonce"),
            metadata = metadata,
        )
    }

    private fun requiredString(value: Any?, field: String): String {
        val string = value as? String
        require(!string.isNullOrBlank()) { "$field must be a non-blank string" }
        return string
    }

    private fun optionalString(value: Any?): String? {
        val string = value as? String ?: return null
        return string
    }

    private fun requiredLong(value: Any?, field: String): Long {
        return when (value) {
            is Int -> value.toLong()
            is Long -> value
            is Double -> {
                require(value % 1.0 == 0.0) { "$field must be an integer" }
                value.toLong()
            }

            else -> error("$field must be an integer")
        }
    }

    private fun optionalLong(value: Any?, field: String): Long? {
        if (value == null) return null
        return requiredLong(value, field)
    }

    private fun optionalInt(value: Any?, field: String): Int? {
        val longValue = optionalLong(value, field) ?: return null
        require(longValue in Int.MIN_VALUE..Int.MAX_VALUE) { "$field must fit in Int" }
        return longValue.toInt()
    }

    private fun asMap(value: Any?, field: String): Map<String, Any?> {
        require(value is Map<*, *>) { "Expected object for $field" }
        return value.entries.associate { (key, entryValue) ->
            key.toString() to entryValue
        }
    }

    private fun asList(value: Any?, field: String): List<Any?> {
        require(value is List<*>) { "Expected array for $field" }
        return value
    }
}
