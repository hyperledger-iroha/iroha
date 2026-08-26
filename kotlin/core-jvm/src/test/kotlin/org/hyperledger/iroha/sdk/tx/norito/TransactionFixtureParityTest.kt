package org.hyperledger.iroha.sdk.tx.norito

import java.nio.file.Files
import java.security.MessageDigest
import java.util.Base64
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.testing.TestEd25519Keys
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.ExecutableBatchItem
import org.hyperledger.iroha.sdk.core.model.FeeChargeKind
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.JsonValue
import org.hyperledger.iroha.sdk.core.model.TransactionAdmissionIntent
import org.hyperledger.iroha.sdk.core.model.WirePayload
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter
import org.hyperledger.iroha.sdk.tx.MultisigSignature
import org.hyperledger.iroha.sdk.tx.SignedTransaction
import org.hyperledger.iroha.sdk.tx.SignedTransactionHasher
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

class TransactionFixtureParityTest {
    private val adapter = NoritoJavaCodecAdapter(org.hyperledger.iroha.sdk.address.AccountAddress.DEFAULT_I105_DISCRIMINANT)

    @Test
    fun `transaction payload fixtures round-trip with kotlin codec`() {
        for (fixture in AndroidFixtureSupport.loadPayloadFixtures()) {
            val payload = fixture.materializePayload()

            assertEquals(
                fixture.networkId,
                payload.networkId.literal,
                "${fixture.name}: network_id mismatch",
            )
            assertEquals(fixture.authority, payload.authority, "${fixture.name}: authority mismatch")
            assertEquals(
                fixture.creationTimeMs,
                payload.creationTimeMs,
                "${fixture.name}: creation_time_ms mismatch",
            )
            assertEquals(
                fixture.timeToLiveMs,
                payload.timeToLiveMs,
                "${fixture.name}: TTL mismatch",
            )
            assertEquals(fixture.nonce, payload.nonce, "${fixture.name}: nonce mismatch")
            assertEquals(
                TransactionAdmissionIntent.ORDINARY,
                payload.admissionIntent,
                "${fixture.name}: admission intent mismatch",
            )

            val encoded = adapter.encodeTransaction(payload)
            val payloadFrame = AndroidFixtureSupport.decodeCanonicalBase64(
                fixture.payloadBase64,
                "${fixture.name}.payload_base64",
            )
            val framedPayload = decodeCanonicalFrame(
                "${fixture.name}.payload",
                payloadFrame,
                TRANSACTION_PAYLOAD_TYPE,
            )
            assertContentEquals(
                framedPayload,
                encoded,
                "${fixture.name}: encoded payload mismatch",
            )
            assertFailsWith<IllegalArgumentException> {
                NoritoHeader.decode(encoded, null)
            }

            val decoded = adapter.decodeTransaction(encoded)
            assertEquals(payload, decoded, "${fixture.name}: Kotlin payload round-trip mismatch")
        }
    }

    @Test
    fun `typed fee payment fixture preserves pipeline gas limits`() {
        val fixture = AndroidFixtureSupport.loadPayloadFixtures()
            .single { it.name == "typed_fee_payment_gas_limit" }
        val payload = fixture.materializePayload()

        val feePayment = assertIs<FeePaymentIntent.Authority>(payload.feePayment)
        assertEquals(1000L, feePayment.gasLimit)
        val charge = feePayment.chargeLimits.single()
        assertEquals(FeeChargeKind.PIPELINE_GAS, charge.kind)
        assertEquals("7EAD8EFYUx1aVKZPUU1fyKvr8dF1", charge.assetDefinitionId)
        assertEquals("1000", charge.maxAmount)
        assertEquals(null, payload.metadata["gas_asset_id"])
        assertEquals(null, payload.metadata["gas_limit"])
        assertEquals(JsonValue.bool(true), payload.metadata["checked"])
    }

    @Test
    fun `mixed executable fixture preserves instruction call instruction order`() {
        val fixture = AndroidFixtureSupport.loadPayloadFixtures()
            .single { it.name == "mixed_executable_batch" }
        val payload = fixture.materializePayload()

        val batch = assertIs<Executable.Batch>(payload.executable)
        assertEquals(3, batch.entries.size)
        assertIs<ExecutableBatchItem.Instruction>(batch.entries[0])
        val invocation = assertIs<ExecutableBatchItem.ContractCall>(batch.entries[1]).invocation
        assertEquals("run", invocation.entrypoint)
        assertContentEquals(byteArrayOf(1, 2, 3, 4), invocation.arguments)
        assertIs<ExecutableBatchItem.Instruction>(batch.entries[2])
    }

    @Test
    fun `transaction fixture manifest remains canonical for kotlin codec`() {
        val payloadFixtures = AndroidFixtureSupport.loadPayloadFixtures()
        val manifestFixtures = AndroidFixtureSupport.loadManifestFixtures()
        assertEquals(
            payloadFixtures.map { it.name }.toSet(),
            manifestFixtures.map { it.name }.toSet(),
            "transaction_payloads.json and manifest must contain exactly the same fixture names",
        )
        val payloadFixturesByName = payloadFixtures.associateBy { it.name }

        for (fixture in manifestFixtures) {
            val encodedPath = AndroidFixtureSupport.resolveSharedResource(fixture.encodedFile)
            val encodedBytes = Files.readAllBytes(encodedPath)
            val payloadFrameBytes = AndroidFixtureSupport.decodeCanonicalBase64(
                fixture.payloadBase64,
                "${fixture.name}.payload_base64",
            )
            val signedFrameBytes = AndroidFixtureSupport.decodeCanonicalBase64(
                fixture.signedBase64,
                "${fixture.name}.signed_base64",
            )
            val payloadBytes = decodeCanonicalFrame(
                "${fixture.name}.payload",
                payloadFrameBytes,
                TRANSACTION_PAYLOAD_TYPE,
            )
            val signedBytes = decodeCanonicalFrame(
                "${fixture.name}.signed",
                signedFrameBytes,
                SIGNED_TRANSACTION_TYPE,
            )
            assertFailsWith<IllegalArgumentException> {
                NoritoHeader.decode(payloadBytes, null)
            }
            assertFailsWith<IllegalArgumentException> {
                NoritoHeader.decode(signedBytes, null)
            }

            assertEquals(
                fixture.encodedLen,
                encodedBytes.size.toLong(),
                "${fixture.name}: encoded_len mismatch",
            )
            assertEquals(
                fixture.payloadBase64,
                Base64.getEncoder().encodeToString(encodedBytes),
                "${fixture.name}: payload_base64 mismatch vs encoded file",
            )
            assertEquals(
                fixture.signedLen,
                signedFrameBytes.size.toLong(),
                "${fixture.name}: signed_len mismatch",
            )
            assertEquals(
                hex(IrohaHash.prehash(payloadFrameBytes)),
                fixture.payloadHash,
                "${fixture.name}: payload_hash mismatch",
            )
            assertEquals(
                SignedTransactionHasher.hashCanonicalHex(signedBytes),
                fixture.signedHash,
                "${fixture.name}: signed_hash mismatch",
            )

            val payload = adapter.decodeTransaction(payloadBytes)
            assertEquals(
                fixture.networkId,
                payload.networkId.literal,
                "${fixture.name}: network_id mismatch",
            )
            assertEquals(
                normalizeAuthority(fixture.authority),
                normalizeAuthority(payload.authority),
                "${fixture.name}: authority mismatch",
            )
            assertEquals(
                fixture.creationTimeMs,
                payload.creationTimeMs,
                "${fixture.name}: creation_time_ms mismatch",
            )
            assertEquals(
                fixture.timeToLiveMs,
                payload.timeToLiveMs,
                "${fixture.name}: TTL mismatch",
            )
            assertEquals(
                fixture.nonce,
                payload.nonce,
                "${fixture.name}: nonce mismatch",
            )
            assertContentEquals(
                payloadFrameBytes,
                encodedBytes,
                "${fixture.name}: encoded file must retain its mandatory Norito header",
            )
            assertContentEquals(
                payloadBytes,
                adapter.encodeTransaction(payload),
                "${fixture.name}: Kotlin payload re-encoding drift",
            )

            val sourceFixture = checkNotNull(payloadFixturesByName[fixture.name]) {
                "${fixture.name}: manifest fixture missing payload source"
            }
            assertEquals(
                sourceFixture.networkId,
                fixture.networkId,
                "${fixture.name}: manifest network_id mismatch",
            )
            assertEquals(
                sourceFixture.payloadBase64,
                fixture.payloadBase64,
                "${fixture.name}: manifest payload mismatch",
            )

            val signedParts = decodeSignedParts(fixture.name, signedBytes)
            assertContentEquals(
                payloadBytes,
                signedParts.payloadBytes,
                "${fixture.name}: signed payload mismatch",
            )

            val signed = SignedTransaction(
                payloadBytes,
                signedParts.signature,
                byteArrayOf(),
                SIGNED_SCHEMA,
            )
            assertContentEquals(
                signedBytes,
                SignedTransactionEncoder.encode(signed),
                "${fixture.name}: Kotlin signed transaction re-encoding drift",
            )

            val versioned = SignedTransactionEncoder.encodeVersioned(signed)
            assertEquals(
                signedBytes.size + 1,
                versioned.size,
                "${fixture.name}: versioned signed length mismatch",
            )
            assertEquals(
                VERSION_BYTE,
                versioned.first(),
                "${fixture.name}: versioned prefix mismatch",
            )
            assertContentEquals(
                signedBytes,
                versioned.copyOfRange(1, versioned.size),
                "${fixture.name}: versioned signed payload mismatch",
            )
            val decodedSigned = SignedTransactionEncoder.decode(signedBytes)
            assertContentEquals(
                payloadBytes,
                decodedSigned.encodedPayload(),
                "${fixture.name}: decoded signed payload mismatch",
            )
            assertContentEquals(
                signedParts.signature,
                decodedSigned.signature(),
                "${fixture.name}: decoded signature mismatch",
            )
            assertEquals(
                false,
                decodedSigned.multisigSignatures().isPresent,
                "${fixture.name}: unexpected multisig signatures",
            )
            assertContentEquals(
                signedBytes,
                SignedTransactionEncoder.encode(decodedSigned),
                "${fixture.name}: decoded signed transaction re-encoding drift",
            )

            val decodedVersioned = SignedTransactionEncoder.decodeVersioned(versioned)
            assertContentEquals(
                payloadBytes,
                decodedVersioned.encodedPayload(),
                "${fixture.name}: decoded versioned payload mismatch",
            )
            assertContentEquals(
                signedParts.signature,
                decodedVersioned.signature(),
                "${fixture.name}: decoded versioned signature mismatch",
            )
        }
    }

    @Test
    fun `fixture support rejects renamed clones and noncanonical base64`() {
        val first = TransactionManifestFixture(
            name = "first",
            networkId = TEST_NETWORK_ID,
            authority = "authority",
            creationTimeMs = 1,
            timeToLiveMs = 100_000L,
            nonce = null,
            payloadBase64 = "AA==",
            payloadHash = "payload-hash",
            encodedFile = "first.norito",
            encodedLen = 1,
            signedBase64 = "AQ==",
            signedHash = "signed-hash",
            signedLen = 1,
        )
        val renamedClone = first.copy(name = "renamed-clone", encodedFile = "renamed-clone.norito")
        val cloneError = assertFailsWith<IllegalStateException> {
            AndroidFixtureSupport.validateManifestFixtureIdentities(listOf(first, renamedClone))
        }
        assertEquals(true, cloneError.message?.contains("Duplicate fixture payload_hash"))

        for (malformed in listOf("YQ!!", "Y Q==", "YQ=", "YQ===", "YR==")) {
            assertFailsWith<IllegalStateException>("must reject $malformed") {
                AndroidFixtureSupport.decodeCanonicalBase64(malformed, "adversarial.fixture")
            }
        }
    }

    @Test
    fun `payload fixture loader requires canonical source fields and rejects retired encoded alias`() {
        for (field in listOf("payload", "payload_base64")) {
            val error = assertFailsWith<IllegalArgumentException> {
                AndroidFixtureSupport.payloadFixtureFromValue(
                    payloadSourceDescriptor().apply { remove(field) },
                )
            }
            assertEquals(true, error.message?.contains(field), error.message)
        }

        val descriptor = payloadSourceDescriptor().apply {
            this["encoded"] = this["payload_base64"]
        }

        val error = assertFailsWith<IllegalStateException> {
            AndroidFixtureSupport.payloadFixtureFromValue(descriptor)
        }
        assertEquals(true, error.message?.contains("retired encoded alias"), error.message)

        val missingAdmissionIntent = payloadSourceDescriptor()
        payloadObject(missingAdmissionIntent).remove("admission_intent")
        val missingIntentError = assertFailsWith<IllegalArgumentException> {
            AndroidFixtureSupport.payloadFixtureFromValue(missingAdmissionIntent)
        }
        assertTrue(
            missingIntentError.message.orEmpty().contains("admission_intent"),
            missingIntentError.message,
        )

        val queuePlanDescriptor = payloadSourceDescriptor()
        payloadObject(queuePlanDescriptor)["admission_intent"] =
            admissionIntent("queue_plan_synced")
        assertEquals(
            TransactionAdmissionIntent.QUEUE_PLAN_SYNCED,
            AndroidFixtureSupport.payloadFixtureFromValue(queuePlanDescriptor)
                .materializePayload()
                .admissionIntent,
        )

        for (invalidIntent in listOf(
            mapOf("intent" to "ordinary"),
            mapOf("intent" to "ordinary", "value" to 0L),
            admissionIntent("legacy"),
        )) {
            val invalidDescriptor = payloadSourceDescriptor()
            payloadObject(invalidDescriptor)["admission_intent"] = invalidIntent
            assertFailsWith<RuntimeException> {
                AndroidFixtureSupport.payloadFixtureFromValue(invalidDescriptor)
                    .materializePayload()
            }
        }
    }

    @Test
    fun transactionFixtureSchemasRejectChainChainIdAndChainIdSnakeCase() {
        for (field in listOf("chain", "chainId", "chain_id")) {
            val topLevel = payloadSourceDescriptor().apply { this[field] = "legacy" }
            val topLevelError = assertFailsWith<IllegalArgumentException> {
                AndroidFixtureSupport.payloadFixtureFromValue(topLevel)
            }
            assertTrue(
                topLevelError.message.orEmpty().contains("unknown fields: [$field]"),
                topLevelError.message,
            )

            val payload = payloadSourceDescriptor()
            payloadObject(payload)[field] = "legacy"
            val payloadError = assertFailsWith<IllegalArgumentException> {
                AndroidFixtureSupport.payloadFixtureFromValue(payload)
            }
            assertTrue(
                payloadError.message.orEmpty().contains("unknown fields: [$field]"),
                payloadError.message,
            )

            val manifest = manifestDescriptor().apply { this[field] = "legacy" }
            val manifestError = assertFailsWith<IllegalArgumentException> {
                AndroidFixtureSupport.manifestFixtureFromValue(manifest)
            }
            assertTrue(
                manifestError.message.orEmpty().contains("unknown fields: [$field]"),
                manifestError.message,
            )
        }
    }

    @Test
    fun `fixture loaders reject unknown and ambiguous schema fields`() {

        for (field in listOf("network_id", "payload.network_id")) {
            val descriptor = payloadSourceDescriptor()
            if (field == "network_id") {
                descriptor[field] = TEST_NETWORK_ID.lowercase()
            } else {
                payloadObject(descriptor)["network_id"] = TEST_NETWORK_ID.lowercase()
            }
            val error = assertFailsWith<IllegalArgumentException> {
                AndroidFixtureSupport.payloadFixtureFromValue(descriptor)
            }
            assertEquals(
                true,
                error.message?.contains("exact canonical hash encoding"),
                error.message,
            )
        }

        val extraTopLevelError = assertFailsWith<IllegalArgumentException> {
            AndroidFixtureSupport.payloadFixtureFromValue(
                payloadSourceDescriptor().apply { this["unexpected"] = true },
            )
        }
        assertEquals(
            true,
            extraTopLevelError.message?.contains("unknown fields: [unexpected]"),
            extraTopLevelError.message,
        )

        val extraPayloadField = payloadSourceDescriptor()
        payloadObject(extraPayloadField)["unexpected"] = true
        val extraPayloadError = assertFailsWith<IllegalArgumentException> {
            AndroidFixtureSupport.payloadFixtureFromValue(extraPayloadField)
        }
        assertEquals(
            true,
            extraPayloadError.message?.contains("unknown fields: [unexpected]"),
            extraPayloadError.message,
        )

        val ambiguousExecutable = payloadSourceDescriptor()
        payloadObject(ambiguousExecutable)["executable"] = mapOf(
            "Ivm" to "AA==",
            "Instructions" to emptyList<Any?>(),
        )
        val ambiguousError = assertFailsWith<IllegalArgumentException> {
            AndroidFixtureSupport.payloadFixtureFromValue(ambiguousExecutable).materializePayload()
        }
        assertEquals(
            true,
            ambiguousError.message?.contains("exactly one externally tagged variant"),
            ambiguousError.message,
        )

        val unknownExecutable = payloadSourceDescriptor()
        payloadObject(unknownExecutable)["executable"] = mapOf("Legacy" to emptyMap<String, Any?>())
        val unknownError = assertFailsWith<IllegalArgumentException> {
            AndroidFixtureSupport.payloadFixtureFromValue(unknownExecutable).materializePayload()
        }
        assertEquals(
            true,
            unknownError.message?.contains("unknown variant Legacy"),
            unknownError.message,
        )

        val extraManifestField = manifestDescriptor().apply { this["unexpected"] = true }
        val extraManifestError = assertFailsWith<IllegalArgumentException> {
            AndroidFixtureSupport.manifestFixtureFromValue(extraManifestField)
        }
        assertEquals(
            true,
            extraManifestError.message?.contains("unknown fields: [unexpected]"),
            extraManifestError.message,
        )
    }

    @Test
    fun `manifest fixture encoded file is canonical and confined to the fixture root`() {
        val renamedError = assertFailsWith<IllegalArgumentException> {
            AndroidFixtureSupport.manifestFixtureFromValue(
                manifestDescriptor().apply { this["encoded_file"] = "renamed.norito" },
            )
        }
        assertEquals(true, renamedError.message?.contains("must be exactly"), renamedError.message)

        val traversalError = assertFailsWith<IllegalArgumentException> {
            AndroidFixtureSupport.manifestFixtureFromValue(
                manifestDescriptor().apply {
                    this["name"] = "nested/fixture"
                    this["encoded_file"] = "nested/fixture.norito"
                },
            )
        }
        assertEquals(
            true,
            traversalError.message?.contains("fixture-root filename"),
            traversalError.message,
        )
    }

    @Test
    fun `fixture loader retains the canonical direct contract call variant`() {
        val descriptor = payloadSourceDescriptor()
        payloadObject(descriptor)["executable"] = mapOf(
            "ContractCall" to mapOf(
                "contract_address" to
                    "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh",
                "expected_code_hash" to
                    "hash:0E5751C026E543B2E8AB2EB06099DAA1D1E5DF47778F7787FAAB45CDF12FE3A9#6A22",
                "entrypoint" to "run",
                "arguments" to listOf(1L, 2L, 3L, 4L),
            ),
        )

        val executable = assertIs<Executable.ContractCall>(
            AndroidFixtureSupport.payloadFixtureFromValue(descriptor).materializePayload().executable,
        )
        assertEquals("run", executable.invocation.entrypoint)
        assertContentEquals(byteArrayOf(1, 2, 3, 4), executable.invocation.arguments)
    }

    @Test
    fun `fixture loaders require explicit matching positive integer TTL`() {
        fun assertRejected(expected: String, block: () -> Unit) {
            val error = assertFailsWith<IllegalStateException>(block = block)
            assertEquals(true, error.message?.contains(expected), error.message)
        }

        assertEquals(
            100_000L,
            AndroidFixtureSupport.payloadFixtureFromValue(payloadSourceDescriptor()).timeToLiveMs,
        )
        assertEquals(
            100_000L,
            AndroidFixtureSupport.manifestFixtureFromValue(manifestDescriptor()).timeToLiveMs,
        )

        for ((invalid, diagnostic) in listOf(null to "must be an integer", 0L to "must be positive")) {
            assertRejected(diagnostic) {
                AndroidFixtureSupport.payloadFixtureFromValue(
                    payloadSourceDescriptor().apply { this["time_to_live_ms"] = invalid },
                )
            }
            assertRejected(diagnostic) {
                AndroidFixtureSupport.manifestFixtureFromValue(
                    manifestDescriptor().apply { this["time_to_live_ms"] = invalid },
                )
            }
            assertRejected(diagnostic) {
                val descriptor = payloadSourceDescriptor()
                @Suppress("UNCHECKED_CAST")
                val payload = descriptor["payload"] as MutableMap<String, Any?>
                payload["time_to_live_ms"] = invalid
                AndroidFixtureSupport.payloadFixtureFromValue(descriptor)
            }
        }

        assertRejected("is required") {
            AndroidFixtureSupport.payloadFixtureFromValue(
                payloadSourceDescriptor().apply { remove("time_to_live_ms") },
            )
        }
        assertRejected("is required") {
            AndroidFixtureSupport.manifestFixtureFromValue(
                manifestDescriptor().apply { remove("time_to_live_ms") },
            )
        }
        assertRejected("is required") {
            val descriptor = payloadSourceDescriptor()
            @Suppress("UNCHECKED_CAST")
            val payload = descriptor["payload"] as MutableMap<String, Any?>
            payload.remove("time_to_live_ms")
            AndroidFixtureSupport.payloadFixtureFromValue(descriptor)
        }
        assertRejected("values must match") {
            val descriptor = payloadSourceDescriptor()
            @Suppress("UNCHECKED_CAST")
            val payload = descriptor["payload"] as MutableMap<String, Any?>
            payload["time_to_live_ms"] = 99_999L
            AndroidFixtureSupport.payloadFixtureFromValue(descriptor)
        }
    }

    @Test
    fun `signed transaction decoder round-trips multisig signatures`() {
        val payload = AndroidFixtureSupport.loadPayloadFixtures()
            .single { it.name == "transfer_asset" }
            .materializePayload()
        val payloadBytes = adapter.encodeTransaction(payload)
        val memberPublicKey = TestEd25519Keys.publicKey(0x41)
        val memberSignature = ByteArray(64) { ((0x80 + it) and 0xFF).toByte() }
        val multisigSignature = MultisigSignature.fromCurveId(0x01, memberPublicKey, memberSignature)
        val signed = SignedTransaction.builder()
            .setEncodedPayload(payloadBytes)
            .setSignature(ByteArray(64) { (it + 1).toByte() })
            .setPublicKey(ByteArray(0))
            .setSchemaName(SIGNED_SCHEMA)
            .setMultisigSignatures(listOf(multisigSignature))
            .build()

        val encoded = SignedTransactionEncoder.encode(signed)
        val decoded = SignedTransactionEncoder.decode(encoded)

        assertContentEquals(payloadBytes, decoded.encodedPayload())
        assertContentEquals(signed.signature(), decoded.signature())
        val decodedMultisig = assertNotNull(decoded.multisigSignatures().orElse(null))
        val decodedSignature = decodedMultisig.signatures.single()
        assertEquals(0x01, decodedSignature.curveId)
        assertContentEquals(memberPublicKey, decodedSignature.publicKey())
        assertContentEquals(memberSignature, decodedSignature.signature())
        assertContentEquals(encoded, SignedTransactionEncoder.encode(decoded))
    }

    @Test
    fun `signed transaction decoder rejects adversarial envelopes`() {
        val fixture = AndroidFixtureSupport.loadManifestFixtures().first()
        val signedBytes = Base64.getDecoder().decode(fixture.signedBase64)

        assertFailsWith<NoritoException> {
            SignedTransactionEncoder.decode(ByteArray(0))
        }
        assertFailsWith<NoritoException> {
            SignedTransactionEncoder.decode(signedBytes.copyOf(signedBytes.size - 1))
        }
        assertFailsWith<NoritoException> {
            SignedTransactionEncoder.decode(signedBytes + byteArrayOf(0))
        }
        assertFailsWith<NoritoException> {
            SignedTransactionEncoder.decodeVersioned(ByteArray(0))
        }
        assertFailsWith<NoritoException> {
            SignedTransactionEncoder.decodeVersioned(byteArrayOf(0x02.toByte()) + signedBytes)
        }
        assertFailsWith<NoritoException> {
            SignedTransactionEncoder.decodeVersioned(byteArrayOf(VERSION_BYTE))
        }
    }

    @Test
    fun `fixture loader accepts wire instruction entries`() {
        val wirePayload = NoritoCodec.encode(
            "wire-fixture",
            "iroha.test.WirePayload",
            NoritoAdapters.stringAdapter(),
        )
        val fixture = AndroidFixtureSupport.payloadFixtureFromValue(
            mapOf(
                "name" to "wire-instruction-fixture",
                "network_id" to TEST_NETWORK_ID,
                "authority" to sampleAuthority(0x51),
                "creation_time_ms" to 0L,
                "time_to_live_ms" to 100_000L,
                "nonce" to null,
                "payload_base64" to "AA==",
                "payload_hash" to "payload-hash",
                "signed_base64" to "AQ==",
                "signed_hash" to "signed-hash",
                "payload" to mapOf(
                    "network_id" to TEST_NETWORK_ID,
                    "authority" to sampleAuthority(0x51),
                    "creation_time_ms" to 0L,
                    "time_to_live_ms" to 100_000L,
                    "nonce" to null,
                    "fee_payment" to mapOf(
                        "payer" to "authority",
                        "value" to mapOf(
                            "charge_limits" to emptyList<Map<String, Any?>>(),
                            "gas_limit" to null,
                        ),
                    ),
                    "admission_intent" to admissionIntent("ordinary"),
                    "metadata" to emptyMap<String, JsonValue>(),
                    "executable" to mapOf(
                        "Instructions" to listOf(
                            mapOf(
                                "wire_name" to "iroha.custom",
                                "payload_base64" to Base64.getEncoder().encodeToString(wirePayload),
                            ),
                        ),
                    ),
                ),
            ),
        )

        val payload = fixture.materializePayload()
        val executable = assertIs<Executable.Instructions>(payload.executable)
        assertEquals(1, executable.instructions.size)
        val wireInstruction = assertIs<WirePayload>(executable.instructions.single().payload)
        assertEquals("iroha.custom", wireInstruction.wireName)
        assertContentEquals(wirePayload, wireInstruction.payloadBytes)
    }

    @Test
    fun `fixture loader rejects wire instruction arguments`() {
        val wirePayload = NoritoCodec.encode(
            "wire-arguments",
            "iroha.test.WirePayload",
            NoritoAdapters.stringAdapter(),
        )
        val fixture = AndroidFixtureSupport.payloadFixtureFromValue(
            mapOf(
                "name" to "wire-instruction-arguments-fixture",
                "network_id" to TEST_NETWORK_ID,
                "authority" to sampleAuthority(0x52),
                "creation_time_ms" to 0L,
                "time_to_live_ms" to 100_000L,
                "nonce" to null,
                "payload_base64" to "AA==",
                "payload_hash" to "payload-hash",
                "signed_base64" to "AQ==",
                "signed_hash" to "signed-hash",
                "payload" to mapOf(
                    "network_id" to TEST_NETWORK_ID,
                    "authority" to sampleAuthority(0x52),
                    "creation_time_ms" to 0L,
                    "time_to_live_ms" to 100_000L,
                    "nonce" to null,
                    "fee_payment" to mapOf(
                        "payer" to "authority",
                        "value" to mapOf(
                            "charge_limits" to emptyList<Map<String, Any?>>(),
                            "gas_limit" to null,
                        ),
                    ),
                    "admission_intent" to admissionIntent("ordinary"),
                    "metadata" to emptyMap<String, JsonValue>(),
                    "executable" to mapOf(
                        "Instructions" to listOf(
                            mapOf(
                                "arguments" to mapOf(
                                    "wire_name" to "iroha.custom",
                                    "payload_base64" to Base64.getEncoder().encodeToString(wirePayload),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        )

        assertFailsWith<RuntimeException> {
            fixture.materializePayload()
        }
    }

    @Test
    fun `fixture loader rejects missing wire instruction fields`() {
        val fixture = AndroidFixtureSupport.payloadFixtureFromValue(
            mapOf(
                "name" to "missing-wire-fields",
                "network_id" to TEST_NETWORK_ID,
                "authority" to sampleAuthority(0x53),
                "creation_time_ms" to 1_735_000_000_000L,
                "time_to_live_ms" to 100_000L,
                "nonce" to null,
                "payload_base64" to "AA==",
                "payload_hash" to "payload-hash",
                "signed_base64" to "AQ==",
                "signed_hash" to "signed-hash",
                "payload" to mapOf(
                    "network_id" to TEST_NETWORK_ID,
                    "authority" to sampleAuthority(0x53),
                    "creation_time_ms" to 1_735_000_000_000L,
                    "time_to_live_ms" to 100_000L,
                    "nonce" to null,
                    "fee_payment" to mapOf(
                        "payer" to "authority",
                        "value" to mapOf(
                            "charge_limits" to emptyList<Map<String, Any?>>(),
                            "gas_limit" to null,
                        ),
                    ),
                    "admission_intent" to admissionIntent("ordinary"),
                    "metadata" to emptyMap<String, JsonValue>(),
                    "executable" to mapOf(
                        "Instructions" to listOf(
                            mapOf("wire_name" to "iroha.register"),
                        ),
                    ),
                ),
            ),
        )

        assertFailsWith<RuntimeException> {
            fixture.materializePayload()
        }
    }

    private fun payloadSourceDescriptor(): MutableMap<String, Any?> = mutableMapOf(
        "name" to "ttl-payload",
        "network_id" to TEST_NETWORK_ID,
        "authority" to CANONICAL_AUTHORITY,
        "creation_time_ms" to 1L,
        "time_to_live_ms" to 100_000L,
        "nonce" to null,
        "payload_base64" to "AA==",
        "payload_hash" to "payload-hash",
        "signed_base64" to "AQ==",
        "signed_hash" to "signed-hash",
        "payload" to mutableMapOf(
            "network_id" to TEST_NETWORK_ID,
            "authority" to CANONICAL_AUTHORITY,
            "creation_time_ms" to 1L,
            "time_to_live_ms" to 100_000L,
            "nonce" to null,
            "fee_payment" to mapOf(
                "payer" to "authority",
                "value" to mapOf(
                    "charge_limits" to emptyList<Map<String, Any?>>(),
                    "gas_limit" to 1_000L,
                ),
            ),
            "admission_intent" to admissionIntent("ordinary"),
            "metadata" to emptyMap<String, Any?>(),
            "executable" to mapOf("Ivm" to "AA=="),
        ),
    )

    private fun admissionIntent(intent: String): Map<String, Any?> = mapOf(
        "intent" to intent,
        "value" to null,
    )

    private fun manifestDescriptor(): MutableMap<String, Any?> = mutableMapOf(
        "name" to "ttl-manifest",
        "network_id" to TEST_NETWORK_ID,
        "authority" to CANONICAL_AUTHORITY,
        "creation_time_ms" to 1L,
        "time_to_live_ms" to 100_000L,
        "nonce" to null,
        "payload_base64" to "AA==",
        "payload_hash" to "payload-hash",
        "encoded_file" to "ttl-manifest.norito",
        "encoded_len" to 1L,
        "signed_base64" to "AQ==",
        "signed_hash" to "signed-hash",
        "signed_len" to 1L,
    )

    @Suppress("UNCHECKED_CAST")
    private fun payloadObject(descriptor: MutableMap<String, Any?>): MutableMap<String, Any?> =
        descriptor["payload"] as MutableMap<String, Any?>

    private fun decodeSignedParts(name: String, signedBytes: ByteArray): SignedParts {
        val decoder = canonicalDecoder(signedBytes)
        val signatureField = readField(decoder, "$name.signed.signature")
        val payloadField = readField(decoder, "$name.signed.payload")
        val multisigField = readField(decoder, "$name.signed.multisig_signatures")
        require(decoder.remaining() == 0) { "$name: signed transaction has trailing bytes" }

        val signature = decodeSignature(name, signatureField)
        decodeOptionField("$name.signed.multisig_signatures", multisigField)
        return SignedParts(signature = signature, payloadBytes = payloadField)
    }

    private fun decodeSignature(name: String, signatureField: ByteArray): ByteArray {
        val fieldDecoder = canonicalDecoder(signatureField)
        val inner = readField(fieldDecoder, "$name.signed.signature.inner")
        require(fieldDecoder.remaining() == 0) { "$name: signature field has trailing bytes" }
        val decoder = canonicalDecoder(inner)
        val signature = BYTE_VECTOR_ADAPTER.decode(decoder)
        require(decoder.remaining() == 0) { "$name: signature payload has trailing bytes" }
        return signature
    }

    private fun decodeOptionField(name: String, fieldBytes: ByteArray): ByteArray? {
        val decoder = canonicalDecoder(fieldBytes)
        val tag = decoder.readByte()
        return when (tag) {
            0 -> {
                require(decoder.remaining() == 0) { "$name: Option::None has trailing bytes" }
                null
            }

            1 -> {
                val length = decoder.readLength(decoder.compactLenActive())
                require(length <= Int.MAX_VALUE) { "$name: Option payload too large" }
                val payload = decoder.readBytes(length.toInt())
                require(decoder.remaining() == 0) { "$name: Option payload has trailing bytes" }
                payload
            }

            else -> error("$name: invalid Option tag $tag")
        }
    }

    private fun readField(decoder: NoritoDecoder, field: String): ByteArray {
        val length = decoder.readLength(decoder.compactLenActive())
        require(length <= Int.MAX_VALUE) { "$field length too large: $length" }
        return decoder.readBytes(length.toInt())
    }

    private fun canonicalDecoder(payload: ByteArray): NoritoDecoder =
        NoritoDecoder(payload, NoritoCodec.DEFAULT_FLAGS)

    private fun decodeCanonicalFrame(
        name: String,
        frame: ByteArray,
        typeName: String,
    ): ByteArray {
        val decoded = NoritoHeader.decode(frame, schemaHash(typeName))
        require(decoded.header.compression == NoritoHeader.COMPRESSION_NONE) {
            "$name: compressed fixture frames are not canonical"
        }
        require(decoded.header.flags == NoritoHeader.COMPACT_LEN) {
            "$name: fixture frame does not use the exact canonical flags"
        }
        require(frame.size == NoritoHeader.HEADER_LENGTH + decoded.header.payloadLength) {
            "$name: fixture frame does not use the exact zero-padding layout"
        }
        decoded.header.validateChecksum(decoded.payload)
        return decoded.payload
    }

    private fun schemaHash(typeName: String): ByteArray {
        val digest = MessageDigest.getInstance("SHA-256")
        digest.update("norito:v1:type-name\u0000".toByteArray(Charsets.UTF_8))
        return digest.digest(typeName.toByteArray(Charsets.UTF_8)).copyOf(16)
    }

    private fun normalizeAuthority(authority: String?): String? {
        val trimmed = authority?.trim() ?: return null
        if (trimmed.isEmpty()) return trimmed
        val atIndex = trimmed.lastIndexOf('@')
        return if (atIndex > 0) trimmed.substring(0, atIndex) else trimmed
    }

    private fun hex(bytes: ByteArray): String = buildString(bytes.size * 2) {
        for (byte in bytes) {
            append("%02x".format(byte.toInt() and 0xFF))
        }
    }

    private fun sampleAuthority(fill: Int): String = AccountAddress
        .fromAccount(TestEd25519Keys.publicKey(fill), "ed25519")
        .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)

    private data class SignedParts(
        val signature: ByteArray,
        val payloadBytes: ByteArray,
    )

    companion object {
        private const val CANONICAL_AUTHORITY =
            "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
        private const val TEST_NETWORK_ID =
            "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
        private const val SIGNED_SCHEMA = "iroha.transaction.SignedTransaction.v1"
        private const val TRANSACTION_PAYLOAD_TYPE =
            "iroha_data_model::transaction::signed::model::TransactionPayload"
        private const val SIGNED_TRANSACTION_TYPE =
            "iroha_data_model::transaction::signed::model::SignedTransaction"
        private const val VERSION_BYTE: Byte = 0x01
        private val BYTE_VECTOR_ADAPTER: TypeAdapter<ByteArray> = NoritoAdapters.byteVecAdapter()
    }
}
