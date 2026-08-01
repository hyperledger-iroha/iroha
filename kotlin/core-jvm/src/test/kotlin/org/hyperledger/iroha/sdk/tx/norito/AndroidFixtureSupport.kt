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
    val timeToLiveMs: Long,
    val nonce: Long?,
    val payload: Map<String, Any?>,
    val payloadBase64: String,
    val payloadHash: String,
    val signedBase64: String,
    val signedHash: String,
) {
    fun materializePayload(): TransactionPayload = AndroidFixtureSupport.buildPayload(name, payload)
}

internal data class TransactionManifestFixture(
    val name: String,
    val chain: String,
    val authority: String,
    val creationTimeMs: Long,
    val timeToLiveMs: Long,
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
    private const val CANONICAL_FIXTURE_ROOT = "fixtures/norito_rpc"
    private val PAYLOAD_FIXTURE_FIELDS = setOf(
        "name",
        "chain",
        "authority",
        "creation_time_ms",
        "time_to_live_ms",
        "nonce",
        "payload",
        "payload_base64",
        "payload_hash",
        "signed_base64",
        "signed_hash",
    )
    private val PAYLOAD_FIELDS = setOf(
        "chain",
        "authority",
        "creation_time_ms",
        "executable",
        "time_to_live_ms",
        "nonce",
        "fee_payment",
        "metadata",
    )
    private val MANIFEST_FIXTURE_FIELDS = setOf(
        "name",
        "chain",
        "authority",
        "creation_time_ms",
        "time_to_live_ms",
        "nonce",
        "payload_base64",
        "payload_hash",
        "encoded_file",
        "encoded_len",
        "signed_base64",
        "signed_hash",
        "signed_len",
    )
    private val EXECUTABLE_VARIANTS = setOf("Ivm", "Instructions", "ContractCall", "Batch")

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
        val timeToLiveMs = requiredPositiveLong(map, "time_to_live_ms", "$name.time_to_live_ms")
        val nonce = optionalLong(map["nonce"], "$name.nonce")
        check(!map.containsKey("encoded")) {
            "$name: retired encoded alias is not accepted"
        }
        val payload = asMap(map["payload"], "$name.payload")
        val payloadTimeToLiveMs = requiredPositiveLong(
            payload,
            "time_to_live_ms",
            "$name.payload.time_to_live_ms",
        )
        check(payloadTimeToLiveMs == timeToLiveMs) {
            "$name: top-level and payload time_to_live_ms values must match"
        }
        requireExactFields(map, PAYLOAD_FIXTURE_FIELDS, "payload fixture $name")
        requireExactFields(payload, PAYLOAD_FIELDS, "$name.payload")
        require(requiredString(payload["chain"], "$name.payload.chain") == chain) {
            "$name: top-level and payload chain values must match"
        }
        require(requiredString(payload["authority"], "$name.payload.authority") == authority) {
            "$name: top-level and payload authority values must match"
        }
        require(
            requiredLong(payload["creation_time_ms"], "$name.payload.creation_time_ms") ==
                creationTimeMs,
        ) {
            "$name: top-level and payload creation_time_ms values must match"
        }
        require(optionalLong(payload["nonce"], "$name.payload.nonce") == nonce) {
            "$name: top-level and payload nonce values must match"
        }
        return TransactionPayloadFixture(
            name = name,
            chain = chain,
            authority = authority,
            creationTimeMs = creationTimeMs,
            timeToLiveMs = timeToLiveMs,
            nonce = nonce,
            payload = payload,
            payloadBase64 = requiredString(map["payload_base64"], "$name.payload_base64"),
            payloadHash = requiredString(map["payload_hash"], "$name.payload_hash"),
            signedBase64 = requiredString(map["signed_base64"], "$name.signed_base64"),
            signedHash = requiredString(map["signed_hash"], "$name.signed_hash"),
        )
    }

    fun loadManifestFixtures(): List<TransactionManifestFixture> {
        val path = resolveSharedResource("transaction_fixtures.manifest.json")
        val parsed = JsonParser.parse(path.readText())
        val manifest = asMap(parsed, path.toString())
        requireExactFields(manifest, setOf("fixtures"), "manifest")
        val fixtures = asList(manifest["fixtures"], "manifest.fixtures").map {
            manifestFixtureFromValue(it)
        }
        validateManifestFixtureIdentities(fixtures)
        return fixtures
    }

    internal fun manifestFixtureFromValue(value: Any?): TransactionManifestFixture {
        val map = asMap(value, "manifest.fixture")
        val name = requiredString(map["name"], "manifest.fixture.name")
        val chain = requiredString(map["chain"], "$name.chain")
        val authority = requiredString(map["authority"], "$name.authority")
        val creationTimeMs = requiredLong(map["creation_time_ms"], "$name.creation_time_ms")
        val timeToLiveMs = requiredPositiveLong(map, "time_to_live_ms", "$name.time_to_live_ms")
        val nonce = optionalLong(map["nonce"], "$name.nonce")
        val payloadBase64 = requiredString(map["payload_base64"], "$name.payload_base64")
        val payloadHash = requiredString(map["payload_hash"], "$name.payload_hash")
        val encodedFile = requiredString(map["encoded_file"], "$name.encoded_file")
        val encodedLen = requiredLong(map["encoded_len"], "$name.encoded_len")
        val signedBase64 = requiredString(map["signed_base64"], "$name.signed_base64")
        val signedHash = requiredString(map["signed_hash"], "$name.signed_hash")
        val signedLen = requiredLong(map["signed_len"], "$name.signed_len")
        requireExactFields(map, MANIFEST_FIXTURE_FIELDS, "manifest fixture $name")
        require(encodedFile == "$name.norito") {
            "$name.encoded_file must be exactly $name.norito"
        }
        require('/' !in encodedFile && '\\' !in encodedFile) {
            "$name.encoded_file must be a fixture-root filename"
        }
        return TransactionManifestFixture(
            name = name,
            chain = chain,
            authority = authority,
            creationTimeMs = creationTimeMs,
            timeToLiveMs = timeToLiveMs,
            nonce = nonce,
            payloadBase64 = payloadBase64,
            payloadHash = payloadHash,
            encodedFile = encodedFile,
            encodedLen = encodedLen,
            signedBase64 = signedBase64,
            signedHash = signedHash,
            signedLen = signedLen,
        )
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
            val candidate = current.resolve(CANONICAL_FIXTURE_ROOT).resolve(name)
            if (Files.exists(candidate)) {
                return candidate
            }
            val parent = current.parent ?: break
            current = parent
        }
        error("Unable to locate $CANONICAL_FIXTURE_ROOT/$name from ${Paths.get("").toAbsolutePath()}")
    }

    internal fun buildPayload(name: String, payload: Map<String, Any?>): TransactionPayload {
        val executableMap = asMap(payload["executable"], "$name.payload.executable")
        require(executableMap.size == 1) {
            "$name.payload.executable must contain exactly one externally tagged variant"
        }
        val (variant, value) = executableMap.entries.single()
        require(variant in EXECUTABLE_VARIANTS) {
            "$name.payload.executable has unknown variant $variant"
        }
        val executable = when (variant) {
            "Ivm" -> {
                val bytes = decodeCanonicalBase64(
                    requiredString(value, "$name.payload.executable.Ivm"),
                    "$name.payload.executable.Ivm",
                )
                Executable.ivm(bytes)
            }

            "Instructions" -> {
                val instructions = asList(
                    value,
                    "$name.payload.executable.Instructions",
                ).mapIndexed { index, raw ->
                    parseInstruction(raw, "$name.payload.executable.Instructions[$index]", name)
                }
                Executable.instructions(instructions)
            }

            "ContractCall" -> Executable.contractCall(
                parseContractInvocation(
                    value,
                    "$name.payload.executable.ContractCall",
                ),
            )

            "Batch" -> {
                val entries = asList(
                    value,
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

            else -> error("$name.payload.executable has unknown variant $variant")
        }

        val metadata = asMap(payload["metadata"], "$name.payload.metadata").mapValues { (_, value) ->
            jsonValue(value)
        }

        return TransactionPayload(
            chainId = requiredString(payload["chain"], "$name.payload.chain"),
            authority = requiredString(payload["authority"], "$name.payload.authority"),
            creationTimeMs = requiredLong(payload["creation_time_ms"], "$name.payload.creation_time_ms"),
            executable = executable,
            timeToLiveMs = requiredPositiveLong(
                payload,
                "time_to_live_ms",
                "$name.payload.time_to_live_ms",
            ),
            nonce = optionalLong(payload["nonce"], "$name.payload.nonce"),
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

    private fun requiredPositiveLong(
        map: Map<String, Any?>,
        key: String,
        field: String,
    ): Long {
        check(map.containsKey(key)) { "$field is required" }
        val value = when (val raw = map[key]) {
            is Byte -> raw.toLong()
            is Short -> raw.toLong()
            is Int -> raw.toLong()
            is Long -> raw
            else -> error("$field must be an integer")
        }
        check(value > 0) { "$field must be positive" }
        return value
    }

    private fun asMap(value: Any?, field: String): Map<String, Any?> {
        require(value is Map<*, *>) { "Expected object for $field" }
        require(value.keys.all { it is String }) { "$field keys must be strings" }
        @Suppress("UNCHECKED_CAST")
        return value as Map<String, Any?>
    }

    private fun asList(value: Any?, field: String): List<Any?> {
        require(value is List<*>) { "Expected array for $field" }
        return value
    }

    private fun requireExactFields(
        value: Map<String, Any?>,
        expected: Set<String>,
        field: String,
    ) {
        val unknown = value.keys - expected
        require(unknown.isEmpty()) { "$field contains unknown fields: ${unknown.sorted()}" }
        val missing = expected - value.keys
        require(missing.isEmpty()) { "$field is missing required fields: ${missing.sorted()}" }
    }
}
