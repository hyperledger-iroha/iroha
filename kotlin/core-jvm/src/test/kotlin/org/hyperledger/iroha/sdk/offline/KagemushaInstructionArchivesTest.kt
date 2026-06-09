package org.hyperledger.iroha.sdk.offline

import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.util.Base64
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.WirePayload
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs

class KagemushaInstructionArchivesTest {
    @Test
    fun `instructionBox preserves redeem archive bytes and wire name`() {
        val archive = kagemushaArchive(KagemushaInstructionType.REDEEM_RECURSIVE)
        val box = KagemushaInstructionArchives.recursiveRedeemInstructionBox(archive)
        archive[0] = 0

        val payload = assertIs<WirePayload>(box.payload)
        assertEquals(
            "iroha_data_model::isi::offline::RedeemKagemushaRecursive",
            payload.wireName,
        )
        assertContentEquals(kagemushaArchive(KagemushaInstructionType.REDEEM_RECURSIVE), payload.payloadBytes)
    }

    @Test
    fun `transactionPayload wraps a single transfer archive instruction`() {
        val archive = kagemushaArchive(KagemushaInstructionType.TRANSFER)
        val payload = KagemushaInstructionArchives.transactionPayload(
            instructionType = KagemushaInstructionType.TRANSFER,
            instructionArchive = archive,
            chainId = "00000042",
            authority = sampleAuthority(),
            creationTimeMs = 1_735_000_000_000L,
            timeToLiveMs = 3_500L,
            nonce = 17,
            metadata = mapOf("mode" to "kagemusha"),
        )

        val executable = assertIs<Executable.Instructions>(payload.executable)
        val box = executable.instructions.single()
        val wire = assertIs<WirePayload>(box.payload)
        assertEquals("iroha_data_model::isi::offline::KagemushaTransfer", wire.wireName)
        assertContentEquals(archive, wire.payloadBytes)
        assertEquals("kagemusha", payload.metadata["mode"])
    }

    @Test
    fun `instructionBox accepts native ABI 7 redeem instruction fixture`() {
        val archive = sharedRecursiveSpendAbi7Archive("redeem_instruction")
        val box = KagemushaInstructionArchives.recursiveRedeemInstructionBox(archive)
        val payload = assertIs<WirePayload>(box.payload)

        assertEquals("iroha_data_model::isi::offline::RedeemKagemushaRecursive", payload.wireName)
        assertContentEquals(archive, payload.payloadBytes)
    }

    @Test
    fun `instructionBox rejects malformed wrong schema empty and tampered archives`() {
        assertFailsWith<IllegalArgumentException> {
            KagemushaInstructionArchives.recursiveRedeemInstructionBox(byteArrayOf())
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaInstructionArchives.recursiveRedeemInstructionBoxFromRequest(byteArrayOf())
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaInstructionArchives.recursiveRedeemTransactionPayloadFromRequest(
                redeemRequestArchive = byteArrayOf(),
                chainId = "00000042",
                authority = sampleAuthority(),
                creationTimeMs = 1_735_000_000_000L,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaInstructionArchives.recursiveRedeemInstructionBox(byteArrayOf(0))
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaInstructionArchives.recursiveRedeemInstructionBox(
                NoritoCodec.encode(
                    "request",
                    "KagemushaRecursiveSpendRedeemRequestV1",
                    NoritoAdapters.stringAdapter(),
                ),
            )
        }

        val tampered = kagemushaArchive(KagemushaInstructionType.REDEEM_RECURSIVE)
        tampered[tampered.lastIndex] = (tampered[tampered.lastIndex].toInt() xor 0x01).toByte()
        assertFailsWith<IllegalArgumentException> {
            KagemushaInstructionArchives.recursiveRedeemInstructionBox(tampered)
        }
    }

    private fun kagemushaArchive(instructionType: KagemushaInstructionType): ByteArray =
        NoritoCodec.encode(
            "payload",
            instructionType.wireName,
            NoritoAdapters.stringAdapter(),
        )

    private fun sampleAuthority(): String = AccountAddress
        .fromAccount(ByteArray(32) { 0x2a.toByte() }, "ed25519")
        .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)

    @Suppress("UNCHECKED_CAST")
    private fun sharedRecursiveSpendAbi7Archive(name: String): ByteArray {
        val root = JsonParser.parse(sharedRecursiveSpendAbi7Fixture("archives.json")) as Map<String, Any?>
        val archives = root["archives"] as List<Map<String, Any?>>
        val archive = archives.first { it["name"] == name }
        return Base64.getDecoder().decode(archive["bytes_base64"] as String)
    }

    private fun sharedRecursiveSpendAbi7Fixture(fileName: String): String {
        var directory: Path? = Paths.get("").toAbsolutePath()
        while (directory != null) {
            val candidate = directory.resolve("fixtures/kagemusha_recursive_spend_abi7").resolve(fileName)
            if (Files.isRegularFile(candidate)) {
                return String(Files.readAllBytes(candidate), Charsets.UTF_8)
            }
            directory = directory.parent
        }
        error("missing shared recursive spend ABI-7 fixture $fileName")
    }
}
