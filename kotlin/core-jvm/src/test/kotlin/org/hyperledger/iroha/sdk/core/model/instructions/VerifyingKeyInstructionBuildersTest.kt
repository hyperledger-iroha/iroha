package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.core.model.zk.VerifyingKeyBackendTag
import org.hyperledger.iroha.sdk.core.model.zk.VerifyingKeyRecordDescription
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class VerifyingKeyInstructionBuildersTest {

    @Test
    fun `register verifying key builder accepts only production verifier backends`() {
        val instruction = RegisterVerifyingKeyInstruction.builder()
            .setBackend("halo2/ipa")
            .setName("treasury-spend")
            .setRecord(sampleRecord("halo2/ipa"))
            .build()

        assertEquals("halo2/ipa", instruction.backend)
        assertEquals("treasury-spend", instruction.name)
        assertEquals("RegisterVerifyingKey", instruction.arguments["action"])
        assertEquals("halo2/ipa", instruction.arguments["backend"])
        assertEquals("treasury-spend", instruction.arguments["name"])
        assertEquals(instruction, RegisterVerifyingKeyInstruction.fromArguments(instruction.arguments))
    }

    @Test
    fun `update verifying key instruction accepts only production verifier backends`() {
        val instruction = UpdateVerifyingKeyInstruction(
            backend = "stark/fri/sha256-goldilocks",
            name = "stark-proof",
            record = sampleRecord("stark/fri/sha256-goldilocks"),
        )

        assertEquals("UpdateVerifyingKey", instruction.arguments["action"])
        assertEquals("stark/fri/sha256-goldilocks", instruction.arguments["backend"])
        assertEquals(instruction, UpdateVerifyingKeyInstruction.fromArguments(instruction.arguments))
    }

    @Test
    fun `register and update reject unsupported production verifier backends`() {
        val record = sampleRecord("halo2/ipa")

        for (backend in unsafeBackends.filter { it.isNotEmpty() }) {
            assertFailsWith<IllegalArgumentException>(backend) {
                RegisterVerifyingKeyInstruction.builder()
                    .setBackend(backend)
                    .setName("vk")
                    .setRecord(record)
                    .build()
            }
            assertFailsWith<IllegalArgumentException>(backend) {
                RegisterVerifyingKeyInstruction(backend, "vk", record)
            }
            assertFailsWith<IllegalArgumentException>(backend) {
                UpdateVerifyingKeyInstruction(backend, "vk", record)
            }
        }
    }

    @Test
    fun `register and update reject inline records committed with a different backend`() {
        val mismatchedRecord = sampleRecord("stark/fri/sha256-goldilocks")

        assertFailsWith<IllegalArgumentException> {
            RegisterVerifyingKeyInstruction("halo2/ipa", "vk", mismatchedRecord)
        }
        assertFailsWith<IllegalArgumentException> {
            UpdateVerifyingKeyInstruction("halo2/ipa", "vk", mismatchedRecord)
        }
    }

    @Test
    fun `fromArguments rejects unsafe backend labels before decoding records`() {
        for (backend in unsafeBackends) {
            val arguments = baseArguments(backend)
            assertFailsWith<IllegalArgumentException>(backend) {
                RegisterVerifyingKeyInstruction.fromArguments(arguments)
            }
            assertFailsWith<IllegalArgumentException>(backend) {
                UpdateVerifyingKeyInstruction.fromArguments(arguments)
            }
        }
    }

    @Test
    fun `fromArguments rejects noncanonical record fields before decoding records`() {
        val canonicalArguments = baseArguments("halo2/ipa")
        for ((key, value) in listOf(
            "record.circuit_id" to " vk-test",
            "record.circuit_id" to "vk-test ",
            "record.backend_tag" to " halo2-ipa-pasta",
            "record.backend_tag" to "HALO2-IPA-PASTA",
            "record.curve" to " pallas",
            "record.curve" to "pallas ",
            "record.public_inputs_schema_hash_hex" to " ${canonicalArguments.getValue("record.public_inputs_schema_hash_hex")}",
            "record.public_inputs_schema_hash_hex" to "${canonicalArguments.getValue("record.public_inputs_schema_hash_hex")} ",
            "record.commitment_hex" to " ${canonicalArguments.getValue("record.commitment_hex")}",
            "record.commitment_hex" to "${canonicalArguments.getValue("record.commitment_hex")} ",
            "record.vk_bytes_b64" to " ${canonicalArguments.getValue("record.vk_bytes_b64")}",
            "record.vk_bytes_b64" to "${canonicalArguments.getValue("record.vk_bytes_b64")} ",
            "record.vk_len" to " ${canonicalArguments.getValue("record.vk_len")}",
            "record.max_proof_bytes" to " 1024",
            "record.gas_schedule_id" to " default",
            "record.gas_schedule_id" to "default ",
            "record.metadata_uri_cid" to " bafy-metadata",
            "record.metadata_uri_cid" to "bafy-metadata ",
            "record.vk_bytes_cid" to " bafy-vk",
            "record.vk_bytes_cid" to "bafy-vk ",
            "record.activation_height" to " 10",
            "record.withdraw_height" to "10 ",
            "record.deprecation_height" to " 10",
            "record.status" to " Active",
            "record.status" to "active",
        )) {
            val registerArguments = baseArguments("halo2/ipa").also { it[key] = value }
            assertFailsWith<IllegalArgumentException>(key) {
                RegisterVerifyingKeyInstruction.fromArguments(registerArguments)
            }

            val updateArguments = baseArguments("halo2/ipa").also { it[key] = value }
            assertFailsWith<IllegalArgumentException>(key) {
                UpdateVerifyingKeyInstruction.fromArguments(updateArguments)
            }
        }
    }

    @Test
    fun `register and update reject blank or padded verifying key names`() {
        val record = sampleRecord("halo2/ipa")

        for (name in listOf("", "   ", "\t", "\n", " vk", "vk ")) {
            assertFailsWith<IllegalArgumentException>(name) {
                RegisterVerifyingKeyInstruction.builder()
                    .setBackend("halo2/ipa")
                    .setName(name)
                    .setRecord(record)
                    .build()
            }
            assertFailsWith<IllegalArgumentException>(name) {
                RegisterVerifyingKeyInstruction("halo2/ipa", name, record)
            }
            assertFailsWith<IllegalArgumentException>(name) {
                UpdateVerifyingKeyInstruction("halo2/ipa", name, record)
            }

            val registerArguments = baseArguments("halo2/ipa").also { it["name"] = name }
            assertFailsWith<IllegalArgumentException>(name) {
                RegisterVerifyingKeyInstruction.fromArguments(registerArguments)
            }

            val updateArguments = baseArguments("halo2/ipa").also { it["name"] = name }
            assertFailsWith<IllegalArgumentException>(name) {
                UpdateVerifyingKeyInstruction.fromArguments(updateArguments)
            }
        }
    }

    private fun sampleRecord(backend: String): VerifyingKeyRecordDescription =
        VerifyingKeyRecordDescription.create(
            backend = backend,
            version = 1,
            circuitId = "vk-test",
            schemaHashHex = "a".repeat(64),
            gasScheduleId = "default",
            backendTag = VerifyingKeyBackendTag.HALO2_IPA_PASTA,
            inlineKeyBytes = byteArrayOf(1, 2, 3),
        )

    private fun baseArguments(backend: String): MutableMap<String, String> =
        RegisterVerifyingKeyInstruction("halo2/ipa", "vk", sampleRecord("halo2/ipa"))
            .arguments
            .toMutableMap()
            .also { it["backend"] = backend }

    private val unsafeBackends = listOf(
        "",
        "halo2/bn254",
        "groth16",
        "groth16/bls12-377",
        " halo2/ipa",
        "halo2/ipa ",
        "\thalo2/ipa",
        "halo2/ipa\n",
        "halo2\uFF0Fipa",
        "halo2/\u200Bipa",
        "h\u0430lo2/ipa",
        " stark/fri/sha256-goldilocks",
        "stark/fri/sha256-goldilocks ",
        "halo2/ipa/orchard",
        "halo2-ipa-orchard",
        "halo2/ipa/penumbra",
        "halo2/ipa/masp",
        "halo2/ipa/monero",
        "halo2/ipa/curve-tree",
        "halo2/pasta/tiny-add",
        "halo2/ipa/tiny-add",
        "halo2/ipa:tiny-add",
        "halo2/pasta/tiny-commit-open",
        "halo2/pasta/anon-transfer-2x2",
        "halo2/ipa/anon-transfer-2x2",
        "halo2/ipa:anon-transfer-2x2",
        "halo2/pasta/anon-transfer-2x2-merkle2",
        "halo2/ipa/anon-transfer-2x2-merkle8",
        "halo2/ipa:anon-transfer-2x2-merkle16",
        "halo2/pasta/vote-bool-commit",
        "halo2/ipa/vote-bool-commit",
        "halo2/ipa:vote-bool-commit",
        "halo2/pasta/vote-bool-commit-merkle2",
        "halo2/ipa/vote-bool-commit-merkle8",
        "halo2/ipa:vote-bool-commit-merkle16",
        "halo2/pasta/asset-hidden-transfer-public-test",
        "halo2/ipa/asset-hidden-transfer-public-test",
        "halo2/ipa:asset-hidden-transfer-public-test",
        "stark/fri/miden/claimed-production",
        "stark/fri/latest",
        "stark/fri/attestation",
        "stark/fri/contest",
        "stark/fri/dev-fixture",
        "stark/fri/d-e-v-f-i-x-t-u-r-e",
        "stark/fri/dev",
        "stark/fri/d-e-v",
        "stark/fri/test",
        "stark/fri/t-e-s-t",
        "stark/fri/todo",
        "stark/fri/t-o-d-o",
        "stark/fri/draft-only",
        "stark/fri/d-r-a-f-t",
        "stark/fri/pending-audit",
        "stark/fri/replace-before-mainnet",
        "stark/fri/not-production-ready",
        "stark/fri/placeholder",
        "halo2/ipa:dev-fixture",
        "halo2/ipa:dev",
        "halo2/ipa:d-e-v",
        "halo2/ipa:todo-proof",
        "halo2/ipa:t-o-d-o-proof",
        "halo2/ipa:draft-proof",
        "halo2/ipa:d-r-a-f-t-proof",
        "halo2/ipa:pending-audit",
        "halo2/ipa:replace-before-production",
        "halo2/ipa:not-for-production",
        "halo2/ipa:dummy",
        "halo2/ipa:f-a-k-e",
        "halo2/ipa:stub",
        "halo2/ipa:s-a-m-p-l-e",
        "halo2/pasta/mock",
        "halo2/pasta/debug-vote",
        "kzg/powersoftau",
        "../halo2/ipa",
        "halo2/ipa\u0000",
    )
}
