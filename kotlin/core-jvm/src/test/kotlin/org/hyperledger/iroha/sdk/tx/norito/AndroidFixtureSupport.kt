package org.hyperledger.iroha.sdk.tx.norito

import java.nio.ByteBuffer
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.util.Base64
import kotlin.io.path.readText
import org.hyperledger.iroha.sdk.client.FeePaymentJson
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.core.model.ContractInvocation
import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.ExecutableBatchItem
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.core.model.JsonValue
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.core.util.HashLiteral

internal data class TransactionPayloadFixture(
    val name: String,
    val chain: String,
    val authority: String,
    val creationTimeMs: Long,
    val timeToLiveMs: Long?,
    val nonce: Int?,
    val payload: Map<String, Any?>?,
    val encodedBase64: String?,
    val payloadHash: String?,
    val signedBase64: String?,
    val signedHash: String?,
) {
    fun materializePayload(adapter: NoritoJavaCodecAdapter): TransactionPayload {
        if (name == LEGACY_GASLESS_IVM_FIXTURE && encodedBase64 != null) {
            return adapter.decodeTransaction(
                AndroidFixtureSupport.decodeCanonicalBase64(encodedBase64, "$name.encoded"),
            )
        }
        payload?.let { return AndroidFixtureSupport.buildPayload(name, it) }
        check(!encodedBase64.isNullOrBlank()) { "$name: fixture missing payload and encoded data" }
        return adapter.decodeTransaction(
            AndroidFixtureSupport.decodeCanonicalBase64(encodedBase64, "$name.encoded"),
        )
    }
}

internal const val LEGACY_GASLESS_IVM_FIXTURE = "ivm_transfer"

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
        val parsed = JsonParser.parse(path.readText())
        val fixtures = asList(parsed, path.toString()).map { entry ->
            payloadFixtureFromValue(entry)
        }
        validatePayloadFixtureIdentities(fixtures)
        return fixtures
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
            payloadHash = optionalString(map["payload_hash"]),
            signedBase64 = optionalString(map["signed_base64"]),
            signedHash = optionalString(map["signed_hash"]),
        )
    }

    fun loadManifestFixtures(): List<TransactionManifestFixture> {
        val path = resolveSharedResource("transaction_fixtures.manifest.json")
        val parsed = JsonParser.parse(path.readText())
        val manifest = asMap(parsed, path.toString())
        val fixtures = asList(manifest["fixtures"], "manifest.fixtures").map { entry ->
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
        validateManifestFixtureIdentities(fixtures)
        return fixtures
    }

    internal fun decodeCanonicalBase64(value: String, field: String): ByteArray {
        val decoded = try {
            Base64.getDecoder().decode(value)
        } catch (ex: IllegalArgumentException) {
            throw IllegalStateException("$field is not valid base64", ex)
        }
        check(Base64.getEncoder().encodeToString(decoded) == value) {
            "$field is not canonical base64"
        }
        return decoded
    }

    internal fun validatePayloadFixtureIdentities(fixtures: List<TransactionPayloadFixture>) {
        val names = mutableSetOf<String>()
        val payloadHashes = mutableSetOf<String>()
        val payloadBytes = mutableSetOf<ByteBuffer>()
        val signedHashes = mutableSetOf<String>()
        val signedBytes = mutableSetOf<ByteBuffer>()
        for (fixture in fixtures) {
            check(names.add(fixture.name)) { "Duplicate fixture name: ${fixture.name}" }
            val encoded = checkNotNull(fixture.encodedBase64) {
                "${fixture.name}: fixture missing encoded payload"
            }
            val payloadHash = checkNotNull(fixture.payloadHash) {
                "${fixture.name}: fixture missing payload_hash"
            }
            val signedBase64 = checkNotNull(fixture.signedBase64) {
                "${fixture.name}: fixture missing signed_base64"
            }
            val signedHash = checkNotNull(fixture.signedHash) {
                "${fixture.name}: fixture missing signed_hash"
            }
            check(payloadHashes.add(payloadHash)) { "Duplicate fixture payload_hash: $payloadHash" }
            check(
                payloadBytes.add(
                    ByteBuffer.wrap(decodeCanonicalBase64(encoded, "${fixture.name}.encoded"))
                        .asReadOnlyBuffer(),
                ),
            ) { "Duplicate fixture payload bytes: ${fixture.name}" }
            check(signedHashes.add(signedHash)) { "Duplicate fixture signed_hash: $signedHash" }
            check(
                signedBytes.add(
                    ByteBuffer.wrap(
                        decodeCanonicalBase64(signedBase64, "${fixture.name}.signed_base64"),
                    ).asReadOnlyBuffer(),
                ),
            ) { "Duplicate fixture signed bytes: ${fixture.name}" }
        }
    }

    internal fun validateManifestFixtureIdentities(fixtures: List<TransactionManifestFixture>) {
        val names = mutableSetOf<String>()
        val encodedFiles = mutableSetOf<String>()
        val payloadHashes = mutableSetOf<String>()
        val payloadBytes = mutableSetOf<ByteBuffer>()
        val signedHashes = mutableSetOf<String>()
        val signedBytes = mutableSetOf<ByteBuffer>()
        for (fixture in fixtures) {
            check(names.add(fixture.name)) { "Duplicate fixture name: ${fixture.name}" }
            check(encodedFiles.add(fixture.encodedFile)) {
                "Duplicate fixture encoded_file: ${fixture.encodedFile}"
            }
            check(payloadHashes.add(fixture.payloadHash)) {
                "Duplicate fixture payload_hash: ${fixture.payloadHash}"
            }
            check(
                payloadBytes.add(
                    ByteBuffer.wrap(
                        decodeCanonicalBase64(
                            fixture.payloadBase64,
                            "${fixture.name}.payload_base64",
                        ),
                    ).asReadOnlyBuffer(),
                ),
            ) { "Duplicate fixture payload bytes: ${fixture.name}" }
            check(signedHashes.add(fixture.signedHash)) {
                "Duplicate fixture signed_hash: ${fixture.signedHash}"
            }
            check(
                signedBytes.add(
                    ByteBuffer.wrap(
                        decodeCanonicalBase64(
                            fixture.signedBase64,
                            "${fixture.name}.signed_base64",
                        ),
                    ).asReadOnlyBuffer(),
                ),
            ) { "Duplicate fixture signed bytes: ${fixture.name}" }
        }
    }

    fun resolveSharedResource(name: String): Path {
        var current = Paths.get("").toAbsolutePath().normalize()
        while (true) {
            val candidate = current.resolve(ANDROID_RESOURCE_ROOT).resolve(name)
            if (Files.exists(candidate)) {
                return candidate
            }
            val parent = current.parent ?: break
            current = parent
        }
        error("Unable to locate $ANDROID_RESOURCE_ROOT/$name from ${Paths.get("").toAbsolutePath()}")
    }

    internal fun buildPayload(name: String, payload: Map<String, Any?>): TransactionPayload {
        val executableMap = asMap(payload["executable"], "$name.payload.executable")
        val executable = when {
            executableMap.containsKey("Ivm") -> {
                val bytes = decodeCanonicalBase64(
                    requiredString(executableMap["Ivm"], "$name.payload.executable.Ivm"),
                    "$name.payload.executable.Ivm",
                )
                Executable.ivm(bytes)
            }

            executableMap.containsKey("Instructions") -> {
                val instructions = asList(
                    executableMap["Instructions"],
                    "$name.payload.executable.Instructions",
                ).mapIndexed { index, raw ->
                    parseInstruction(raw, "$name.payload.executable.Instructions[$index]", name)
                }
                Executable.instructions(instructions)
            }

            executableMap.containsKey("ContractCall") -> Executable.contractCall(
                parseContractInvocation(
                    executableMap["ContractCall"],
                    "$name.payload.executable.ContractCall",
                ),
            )

            executableMap.containsKey("Batch") -> {
                val entries = asList(
                    executableMap["Batch"],
                    "$name.payload.executable.Batch",
                ).mapIndexed { index, raw ->
                    val context = "$name.payload.executable.Batch[$index]"
                    val item = asMap(raw, context)
                    require(item.size == 1) {
                        "$context must contain exactly one externally tagged variant"
                    }
                    when {
                        item.containsKey("Instruction") -> ExecutableBatchItem.instruction(
                            parseInstruction(item["Instruction"], "$context.Instruction", name),
                        )

                        item.containsKey("ContractCall") -> ExecutableBatchItem.contractCall(
                            parseContractInvocation(item["ContractCall"], "$context.ContractCall"),
                        )

                        else -> error("$context has an unknown executable batch item variant")
                    }
                }
                Executable.batch(entries)
            }

            else -> error("$name: executable variant missing")
        }

        val metadata = payload["metadata"]?.let { raw ->
            asMap(raw, "$name.payload.metadata").mapValues { (_, value) ->
                jsonValue(value)
            }
        } ?: emptyMap<String, JsonValue>()

        return TransactionPayload(
            chainId = requiredString(payload["chain"], "$name.payload.chain"),
            authority = requiredString(payload["authority"], "$name.payload.authority"),
            creationTimeMs = requiredLong(payload["creation_time_ms"], "$name.payload.creation_time_ms"),
            executable = executable,
            timeToLiveMs = optionalLong(payload["time_to_live_ms"], "$name.payload.time_to_live_ms"),
            nonce = optionalInt(payload["nonce"], "$name.payload.nonce"),
            feePayment = FeePaymentJson.parse(
                payload["fee_payment"],
                "$name.payload.fee_payment",
            ),
            metadata = metadata,
        )
    }

    private fun parseInstruction(value: Any?, context: String, fixtureName: String): InstructionBox {
        val instruction = asMap(value, context)
        require(instruction.size == 2) {
            "$fixtureName: instruction entries must only include wire_name and payload_base64"
        }
        val wireName = requiredString(instruction["wire_name"], "$context.wire_name")
        val payloadBase64 = requiredString(
            instruction["payload_base64"],
            "$context.payload_base64",
        )
        val bytes = try {
            decodeCanonicalBase64(payloadBase64, "$context.payload_base64")
        } catch (ex: IllegalStateException) {
            throw IllegalStateException(
                "$fixtureName: instruction payload_base64 is not valid base64",
                ex,
            )
        }
        return InstructionBox.fromWirePayload(wireName, bytes)
    }

    private fun parseContractInvocation(value: Any?, context: String): ContractInvocation {
        val invocation = asMap(value, context)
        require(invocation.keys == setOf(
            "contract_address",
            "expected_code_hash",
            "entrypoint",
            "arguments",
        ) || invocation.keys == setOf(
            "contract_address",
            "expected_code_hash",
            "entrypoint",
        )) {
            "$context contains unexpected fields"
        }
        val arguments = invocation["arguments"]?.let { raw ->
            asList(raw, "$context.arguments").mapIndexed { index, byte ->
                val value = requiredLong(byte, "$context.arguments[$index]")
                require(value in 0..255) { "$context.arguments[$index] must fit in a byte" }
                value.toByte()
            }.toByteArray()
        }
        return ContractInvocation(
            contractAddress = requiredString(
                invocation["contract_address"],
                "$context.contract_address",
            ),
            expectedCodeHash = HashLiteral.decode(
                requiredString(invocation["expected_code_hash"], "$context.expected_code_hash"),
            ),
            entrypoint = requiredString(invocation["entrypoint"], "$context.entrypoint"),
            arguments = arguments,
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

    private fun jsonValue(value: Any?): JsonValue = when (value) {
        null -> JsonValue.raw("null")
        is String -> JsonValue.string(value)
        is Number -> JsonValue.raw(value.toString())
        is Boolean -> JsonValue.bool(value)
        else -> error("Unsupported metadata JSON value type: ${value::class}")
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
