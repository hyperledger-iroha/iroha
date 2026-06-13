package org.hyperledger.iroha.sdk.core.model.instructions

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import org.hyperledger.iroha.sdk.crypto.NativeSignedTransaction
import org.hyperledger.iroha.sdk.crypto.NativeSignerBridge
import org.hyperledger.iroha.sdk.crypto.SigningAlgorithm

class ZkAssetInstructionsTest {
    @Test
    fun confidentialEncryptedPayloadIsStrictAndDefensive() {
        val ephemeral = fill(0x11, 32)
        val nonce = fill(0x22, 24)
        val ciphertext = byteArrayOf(0x33, 0x34)
        val payload = ConfidentialEncryptedPayload(
            ephemeralPublicKey = ephemeral,
            nonce = nonce,
            ciphertext = ciphertext,
        )

        ephemeral[0] = 0
        nonce[0] = 0
        ciphertext[0] = 0
        assertEquals(ConfidentialEncryptedPayload.VERSION_V1, payload.version)
        assertEquals(0x11, payload.ephemeralPublicKey[0].toInt())
        assertEquals(0x22, payload.nonce[0].toInt())
        assertEquals(0x33, payload.ciphertext[0].toInt())

        val exposed = payload.ephemeralPublicKey
        exposed[0] = 0
        assertEquals(0x11, payload.ephemeralPublicKey[0].toInt())

        assertFailsWith<IllegalArgumentException> {
            ConfidentialEncryptedPayload(2, fill(1, 32), fill(2, 24), byteArrayOf(3))
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialEncryptedPayload(
                ephemeralPublicKey = ByteArray(32),
                nonce = fill(2, 24),
                ciphertext = byteArrayOf(3),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialEncryptedPayload(
                ephemeralPublicKey = ByteArray(32).also { it[0] = 1 },
                nonce = fill(2, 24),
                ciphertext = byteArrayOf(3),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialEncryptedPayload(
                ephemeralPublicKey = fill(1, 31),
                nonce = fill(2, 24),
                ciphertext = byteArrayOf(3),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialEncryptedPayload(
                ephemeralPublicKey = fill(1, 32),
                nonce = fill(2, 23),
                ciphertext = byteArrayOf(3),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialEncryptedPayload(
                ephemeralPublicKey = fill(1, 32),
                nonce = fill(2, 24),
                ciphertext = ByteArray(0),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialEncryptedPayload(
                ephemeralPublicKey = fill(1, 32),
                nonce = fill(2, 24),
                ciphertext = ByteArray(ConfidentialEncryptedPayload.MAX_CIPHERTEXT_BYTES + 1),
            )
        }
    }

    @Test
    fun confidentialEncryptedPayloadMatchesRustWireFixture() {
        val ephemeral = hex("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")
        val nonce = hex("a0a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b7")
        val ciphertext = hex("436f6e666964656e7469616c5061796c6f61645631")
        val serialized = hex(
            "01000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f" +
                "a0a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b715" +
                "436f6e666964656e7469616c5061796c6f61645631",
        )
        val payload = ConfidentialEncryptedPayload(
            ephemeralPublicKey = ephemeral,
            nonce = nonce,
            ciphertext = ciphertext,
        )

        assertContentEquals(serialized, payload.toWireBytes())
        assertEquals(payload, ConfidentialEncryptedPayload.fromWireBytes(serialized))
        val exposedWire = payload.toWireBytes()
        exposedWire[0] = 0
        assertContentEquals(serialized, payload.toWireBytes())

        assertFailsWith<IllegalArgumentException> {
            ConfidentialEncryptedPayload.fromWireBytes(serialized.copyOf(serialized.size - 1))
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialEncryptedPayload.fromWireBytes(serialized + byteArrayOf(0))
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialEncryptedPayload.fromWireBytes(
                byteArrayOf(0) + ephemeral + nonce + byteArrayOf(ciphertext.size.toByte()) + ciphertext,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialEncryptedPayload.fromWireBytes(
                byteArrayOf(1) + ephemeral + nonce + byteArrayOf(0x95.toByte(), 0) + ciphertext,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialEncryptedPayload.fromWireBytes(
                byteArrayOf(1) + ephemeral + nonce +
                    byteArrayOf(0x81.toByte(), 0x80.toByte(), 0x04),
            )
        }
    }

    @Test
    fun proofAttachmentValidatesBackendAndJsonShape() {
        val attachment = ProofAttachment(
            backend = "halo2/ipa",
            proofBytes = byteArrayOf(0x40, 0x41),
            verifyingKeyRef = ProofVerifierKeyRef("halo2/ipa", "unshield-v3"),
            verifyingKeyCommitment = fill(0x55, 32),
            envelopeHash = fill(0x66, 32),
        )

        val json = attachment.toNativeJson()
        assert(json.contains("\"backend\":\"halo2/ipa\""))
        assert(json.contains("\"proof_b64\":\"QEE=\""))
        assert(json.contains("\"vk_ref\":{\"backend\":\"halo2/ipa\",\"name\":\"unshield-v3\"}"))
        assertFalse(json.contains("vk_inline"))

        assertFailsWith<IllegalArgumentException> {
            ProofAttachment(
                backend = "halo2/ipa",
                proofBytes = ByteArray(0),
                verifyingKeyRef = ProofVerifierKeyRef("halo2/ipa", "vk"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            ProofAttachment(
                backend = "halo2/ipa",
                proofBytes = byteArrayOf(1),
                verifyingKeyRef = ProofVerifierKeyRef("stark/fri", "vk"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            ProofAttachment(
                backend = "halo2/ipa",
                proofBytes = byteArrayOf(1),
                verifyingKeyRef = ProofVerifierKeyRef("halo2/ipa", "vk"),
                verifyingKeyCommitment = ByteArray(32),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            ProofVerifierKeyRef.fromWireId("missing-separator")
        }
    }

    @Test
    fun shieldInstructionValidatesCanonicalFieldsAndCopiesBytes() {
        val commitment = fill(0x7a, 32)
        val instruction = ShieldInstruction.builder()
            .setAsset("rose#wonderland")
            .setFrom("alice")
            .setAmount("340282366920938463463374607431768211455")
            .setNoteCommitment(commitment)
            .setEncryptedPayload(samplePayload())
            .build()

        commitment[0] = 0
        assertEquals("Shield", instruction.arguments["action"])
        assertEquals("340282366920938463463374607431768211455", instruction.amount)
        assertEquals(0x7a, instruction.noteCommitment[0].toInt())
        val exposed = instruction.noteCommitment
        exposed[0] = 0
        assertEquals(0x7a, instruction.noteCommitment[0].toInt())

        assertFailsWith<IllegalArgumentException> {
            ShieldInstruction.builder().setAmount("01")
        }
        assertFailsWith<IllegalArgumentException> {
            ShieldInstruction.builder().setAmount("-1")
        }
        assertFailsWith<IllegalArgumentException> {
            ShieldInstruction.builder().setAmount("340282366920938463463374607431768211456")
        }
        assertFailsWith<IllegalArgumentException> {
            ShieldInstruction.builder().setNoteCommitment(ByteArray(32))
        }
    }

    @Test
    fun unshieldInstructionValidatesInputsOutputsAndProof() {
        val input = fill(0x20, 32)
        val output = fill(0x21, 32)
        val root = fill(0x22, 32)
        val instruction = UnshieldInstruction.builder()
            .setAsset("rose#wonderland")
            .setTo("bob")
            .setPublicAmount("0")
            .addInput(input)
            .addOutput(output)
            .setProof(sampleProof())
            .setRootHint(root)
            .build()

        input[0] = 0
        output[0] = 0
        root[0] = 0
        assertEquals("Unshield", instruction.arguments["action"])
        assertEquals("0", instruction.publicAmount)
        assertEquals(1, instruction.inputs.size)
        assertEquals(1, instruction.outputs.size)
        assertEquals(0x20, instruction.inputs[0][0].toInt())
        assertEquals(0x21, instruction.outputs[0][0].toInt())
        assertEquals(0x22, instruction.rootHint!![0].toInt())

        assertFailsWith<IllegalStateException> {
            UnshieldInstruction.builder()
                .setAsset("rose#wonderland")
                .setTo("bob")
                .setPublicAmount("1")
                .setProof(sampleProof())
                .build()
        }
        assertFailsWith<IllegalArgumentException> {
            UnshieldInstruction.builder().addInput(ByteArray(32))
        }
        assertFailsWith<IllegalArgumentException> {
            UnshieldInstruction.builder().addOutput(fill(1, 31))
        }
        assertFailsWith<IllegalArgumentException> {
            UnshieldInstruction.builder().setRootHint(fill(1, 31))
        }
    }

    @Test
    fun registerZkAssetInstructionValidatesModeAndVerifierIds() {
        val instruction = RegisterZkAssetInstruction.builder()
            .setAsset("rose#wonderland")
            .setMode(ZkAssetMode.HYBRID)
            .setAllowShield(true)
            .setAllowUnshield(false)
            .setTransferVerifyingKey("halo2/ipa:transfer-v2")
            .build()

        assertEquals(InstructionKind.REGISTER, instruction.kind)
        assertEquals("Hybrid", instruction.arguments["mode"])
        assertEquals("false", instruction.arguments["allow_unshield"])

        assertEquals(ZkAssetMode.ZK_NATIVE, ZkAssetMode.fromWireName("ZkNative"))
        assertFailsWith<IllegalArgumentException> {
            ZkAssetMode.fromWireName("zk-native")
        }
        assertFailsWith<IllegalArgumentException> {
            RegisterZkAssetInstruction.builder().setTransferVerifyingKey("halo2/ipa")
        }
    }

    @Test
    fun nativeSignerZkMethodsRejectBadInputsBeforeNativeDispatch() {
        val shield = ShieldInstruction.builder()
            .setAsset("rose#wonderland")
            .setFrom("alice")
            .setAmount("1")
            .setNoteCommitment(fill(1, 32))
            .setEncryptedPayload(samplePayload())
            .build()
        val unshield = UnshieldInstruction.builder()
            .setAsset("rose#wonderland")
            .setTo("bob")
            .setPublicAmount("1")
            .addInput(fill(2, 32))
            .setProof(sampleProof())
            .build()
        val register = RegisterZkAssetInstruction.builder()
            .setAsset("rose#wonderland")
            .build()

        assertFailsWith<IllegalArgumentException> {
            NativeSignerBridge.encodeShieldSignedTransaction(
                SigningAlgorithm.ED25519,
                "chain",
                "alice",
                -1,
                null,
                shield,
                byteArrayOf(1),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            NativeSignerBridge.encodeUnshieldSignedTransaction(
                SigningAlgorithm.ED25519,
                " chain ",
                "alice",
                0,
                null,
                unshield,
                byteArrayOf(1),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            NativeSignerBridge.encodeRegisterZkAssetSignedTransaction(
                SigningAlgorithm.ED25519,
                "chain",
                "alice",
                0,
                0L,
                register,
                byteArrayOf(1),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            NativeSignerBridge.encodeShieldSignedTransaction(
                SigningAlgorithm.ED25519,
                "chain",
                "alice",
                0,
                null,
                shield,
                ByteArray(0),
            )
        }
    }

    @Test
    fun nativeSignedTransactionCopiesInputsAndOutputs() {
        val versioned = byteArrayOf(1, 2, 3)
        val hash = fill(0x30, 32)
        val signed = NativeSignedTransaction(versioned, hash)
        versioned[0] = 9
        hash[0] = 9
        assertContentEquals(byteArrayOf(1, 2, 3), signed.versionedSignedTransaction)
        assertEquals(0x30, signed.transactionHash[0].toInt())
        val exposed = signed.versionedSignedTransaction
        exposed[0] = 9
        assertContentEquals(byteArrayOf(1, 2, 3), signed.versionedSignedTransaction)

        assertFailsWith<IllegalArgumentException> {
            NativeSignedTransaction(ByteArray(0), fill(1, 32))
        }
        assertFailsWith<IllegalArgumentException> {
            NativeSignedTransaction(byteArrayOf(1), fill(1, 31))
        }
    }

    private fun samplePayload(): ConfidentialEncryptedPayload =
        ConfidentialEncryptedPayload(
            ephemeralPublicKey = fill(0x11, 32),
            nonce = fill(0x22, 24),
            ciphertext = byteArrayOf(0x33, 0x34),
        )

    private fun sampleProof(): ProofAttachment =
        ProofAttachment(
            backend = "halo2/ipa",
            proofBytes = byteArrayOf(0x44),
            verifyingKeyRef = ProofVerifierKeyRef("halo2/ipa", "unshield-v3"),
            verifyingKeyCommitment = fill(0x55, 32),
        )

    private fun fill(value: Int, size: Int): ByteArray = ByteArray(size) { value.toByte() }

    private fun hex(value: String): ByteArray {
        require(value.length % 2 == 0)
        return ByteArray(value.length / 2) { index ->
            value.substring(index * 2, index * 2 + 2).toInt(16).toByte()
        }
    }
}
