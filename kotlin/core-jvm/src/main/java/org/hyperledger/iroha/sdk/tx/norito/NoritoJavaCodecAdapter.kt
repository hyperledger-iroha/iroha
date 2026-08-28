package org.hyperledger.iroha.sdk.tx.norito

import java.util.Base64
import java.util.Collections
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.client.MultisigProposeRequest
import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.TransactionAdmissionIntent
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.core.model.WirePayload
import org.hyperledger.iroha.sdk.core.util.HashLiteral
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoHeader

class NoritoJavaCodecAdapter @JvmOverloads constructor(
    val chainDiscriminant: Int,
    private val schemaName: String = DEFAULT_SCHEMA,
) : NoritoCodecAdapter {

    private val adapter = TransactionPayloadAdapter.forChain(chainDiscriminant)

    @Throws(NoritoException::class)
    override fun encodeTransaction(payload: TransactionPayload): ByteArray {
        try {
            return NoritoCodec.encodeAdaptive(payload, adapter).payload()
        } catch (ex: Exception) {
            throw NoritoException("Failed to encode Norito transaction payload", ex)
        }
    }

    @Throws(NoritoException::class)
    override fun decodeTransaction(encoded: ByteArray): TransactionPayload {
        try {
            if (hasHeader(encoded)) {
                return NoritoCodec.decode(encoded, adapter, schemaName)
            }
            return NoritoCodec.decodeAdaptive(encoded, adapter)
        } catch (ex: Exception) {
            throw NoritoException("Failed to decode Norito transaction payload", ex)
        }
    }

    override fun schemaName(): String = schemaName

    companion object {
        private const val DEFAULT_SCHEMA = "iroha.android.transaction.Payload.v1"

        @JvmStatic
        @Throws(NoritoException::class)
        fun encodeInstructionBox(instruction: InstructionBox): ByteArray {
            try {
                return TransactionPayloadAdapter.encodeInstructionBox(instruction)
            } catch (ex: Exception) {
                throw NoritoException("Failed to encode Norito instruction box", ex)
            }
        }

        /** Decode one exact canonical wire-framed instruction box. */
        @JvmStatic
        @Throws(NoritoException::class)
        fun decodeInstructionBox(encoded: ByteArray): InstructionBox {
            try {
                val decoded = TransactionPayloadAdapter.decodeInstructionBox(encoded)
                val canonical = TransactionPayloadAdapter.encodeInstructionBox(decoded)
                require(encoded.contentEquals(canonical)) {
                    "instruction box is not canonically encoded"
                }
                return decoded
            } catch (ex: Exception) {
                throw NoritoException("Failed to decode canonical Norito instruction box", ex)
            }
        }

        /**
         * Returns the exact canonical inner instruction frames committed by a multisig proposal.
         *
         * Torii appends one canonical validation-fee marker when the request selects a fee
         * instruction. The returned list is defensive and includes that marker.
         */
        @JvmStatic
        @Throws(NoritoException::class)
        fun canonicalMultisigProposalInstructionBoxes(
            request: MultisigProposeRequest,
        ): List<ByteArray> {
            try {
                require(request.instructions.isNotEmpty()) {
                    "multisig instructions must not be empty"
                }
                val instructions = request.instructions.mapIndexed { index, encoded ->
                    require(encoded.isNotEmpty()) { "multisig instructions[$index] must not be empty" }
                    encoded.copyOf()
                }.toMutableList()
                TransactionPayloadAdapter.encodeCanonicalInstructionBoxes(instructions).fill(0)
                instructions.forEach { encoded ->
                    val instruction = TransactionPayloadAdapter.decodeInstructionBox(encoded)
                    val wire = instruction.payload as? WirePayload
                    if (wire?.wireName == LOG_WIRE_NAME) {
                        val log = TransactionPayloadAdapter.decodeCanonicalLogInstruction(encoded)
                        require(!log.message.startsWith(VALIDATION_FEE_MULTISIG_RESERVED_PREFIX)) {
                            "multisig propose request instructions must not contain a validation-fee marker"
                        }
                    }
                }

                request.validationFeeTransferEntryIndex?.let {
                    require(request.validationFeeInstructionIndex != null) {
                        "validationFeeTransferEntryIndex requires validationFeeInstructionIndex"
                    }
                }
                request.validationFeeInstructionIndex?.let { instructionIndex ->
                    require(instructionIndex >= 0L) {
                        "validationFeeInstructionIndex must be non-negative"
                    }
                    val policyVersion = requireNotNull(request.validationFeePolicyVersion) {
                        "validationFeeInstructionIndex requires validationFeePolicyVersion"
                    }
                    require(policyVersion > 0L) {
                        "validationFeePolicyVersion must be positive when a marker is required"
                    }
                    val policyHash = canonicalLowerHex32(
                        requireNotNull(request.validationFeePolicyHash) {
                            "validationFeeInstructionIndex requires validationFeePolicyHash"
                        },
                        "validationFeePolicyHash",
                    )
                    val hijiriHash = request.validationFeeHijiriFeeQuoteHash?.let {
                        canonicalLowerHex32(it, "validationFeeHijiriFeeQuoteHash")
                    } ?: "-"
                    val transferEntryIndex = request.validationFeeTransferEntryIndex?.let { index ->
                        require(index >= 0L) {
                            "validationFeeTransferEntryIndex must be non-negative"
                        }
                        index.toString()
                    } ?: "-"
                    val message = buildString {
                        append(VALIDATION_FEE_MULTISIG_MARKER_PREFIX)
                        append(policyVersion)
                        append(':')
                        append(policyHash)
                        append(':')
                        append(hijiriHash)
                        append(':')
                        append(instructionIndex)
                        append(':')
                        append(transferEntryIndex)
                    }
                    instructions.add(
                        TransactionPayloadAdapter.encodeCanonicalLogInstruction(
                            TRACE_LEVEL_TAG,
                            message,
                        ),
                    )
                }
                return Collections.unmodifiableList(instructions.map(ByteArray::copyOf))
            } catch (ex: Exception) {
                throw NoritoException("Failed to build canonical multisig proposal instructions", ex)
            }
        }

        /** Rust-compatible `HashOf<Vec<InstructionBox>>` for exact canonical instruction frames. */
        @JvmStatic
        @Throws(NoritoException::class)
        fun hashCanonicalInstructionBoxes(encodedInstructions: List<ByteArray>): ByteArray {
            var preimage: ByteArray? = null
            try {
                val snapshot = snapshotInstructionBoxes(encodedInstructions)
                preimage = TransactionPayloadAdapter.encodeCanonicalInstructionBoxes(snapshot)
                return IrohaHash.prehash(preimage)
            } catch (ex: Exception) {
                throw NoritoException("Failed to hash canonical Norito instruction boxes", ex)
            } finally {
                preimage?.fill(0)
            }
        }

        /**
         * Verifies the exact outer action built by `POST /v1/multisig/propose`.
         *
         * The executable must contain only the canonical propose instruction and optional
         * quorum-one approval. The returned bytes are the locally computed proposal hash.
         */
        @JvmStatic
        @Throws(NoritoException::class)
        fun verifyCanonicalMultisigProposeExecutable(
            transactionPayload: TransactionPayload,
            expectedMultisigAccountId: String,
            expectedInstructionBoxes: List<ByteArray>,
        ): ByteArray {
            val expected = snapshotInstructionBoxes(expectedInstructionBoxes)
            try {
                val canonicalAccount = requireCanonicalI105Address(
                    expectedMultisigAccountId,
                    "expectedMultisigAccountId",
                )
                val executable = transactionPayload.executable as? Executable.Instructions
                    ?: throw IllegalArgumentException(
                        "multisig proposal transaction must contain an instruction executable",
                    )
                require(executable.instructions.size in 1..2) {
                    "multisig proposal transaction must contain only propose and optional approve"
                }
                val instructionsHash = hashCanonicalInstructionBoxes(expected)
                verifyMultisigProposeJson(
                    TransactionPayloadAdapter.decodeCanonicalCustomInstructionJson(
                        executable.instructions[0],
                    ),
                    canonicalAccount,
                    expected,
                )
                if (executable.instructions.size == 2) {
                    verifyMultisigApproveJson(
                        TransactionPayloadAdapter.decodeCanonicalCustomInstructionJson(
                            executable.instructions[1],
                        ),
                        canonicalAccount,
                        HashLiteral.canonicalize(instructionsHash),
                    )
                }
                return instructionsHash
            } catch (ex: Exception) {
                throw NoritoException("Invalid canonical multisig propose executable", ex)
            } finally {
                expected.forEach { it.fill(0) }
            }
        }

        /** Reject transaction payload bytes that are not the exact canonical Norito encoding. */
        @JvmStatic
        @Throws(NoritoException::class)
        fun validateCanonicalTransactionPayload(encoded: ByteArray) {
            decodeCanonicalTransactionPayload(encoded)
        }

        /** Reject non-canonical payloads and payloads with a different admission intent. */
        @JvmStatic
        @Throws(NoritoException::class)
        fun validateCanonicalTransactionPayload(
            encoded: ByteArray,
            expectedAdmissionIntent: TransactionAdmissionIntent,
        ) {
            decodeCanonicalTransactionPayload(encoded, expectedAdmissionIntent)
        }

        /** Decode one exact canonical payload so callers can verify signature-bound fields. */
        @JvmStatic
        @JvmOverloads
        @Throws(NoritoException::class)
        fun decodeCanonicalTransactionPayload(
            encoded: ByteArray,
            expectedAdmissionIntent: TransactionAdmissionIntent? = null,
        ): TransactionPayload {
            try {
                val payload = TransactionPayloadAdapter.validateCanonicalPayloadBytes(encoded)
                if (expectedAdmissionIntent != null) {
                    require(payload.admissionIntent == expectedAdmissionIntent) {
                        "transaction payload admission intent must be $expectedAdmissionIntent"
                    }
                }
                return payload
            } catch (ex: Exception) {
                throw NoritoException("Invalid canonical Norito transaction payload", ex)
            }
        }

        private fun snapshotInstructionBoxes(encodedInstructions: List<ByteArray>): List<ByteArray> {
            require(encodedInstructions.isNotEmpty()) {
                "Expected multisig instruction boxes must not be empty"
            }
            return encodedInstructions.mapIndexed { index, instruction ->
                require(instruction.isNotEmpty()) {
                    "Expected multisig instruction box $index must not be empty"
                }
                instruction.copyOf()
            }
        }

        private fun verifyMultisigProposeJson(
            json: String,
            expectedAccount: String,
            expectedInstructions: List<ByteArray>,
        ) {
            val root = requireJsonObject(JsonParser.parse(json), "multisig custom")
            requireExactKeys(root, setOf("Propose"), "multisig custom")
            val propose = requireJsonObject(root["Propose"], "multisig custom.Propose")
            requireExactKeys(
                propose,
                setOf("account", "instructions", "transaction_ttl_ms"),
                "multisig custom.Propose",
            )
            requireExactString(
                propose["account"],
                expectedAccount,
                "multisig custom.Propose.account",
            )
            require(propose["transaction_ttl_ms"] == null) {
                "multisig custom.Propose.transaction_ttl_ms must be null"
            }
            val embedded = propose["instructions"] as? List<*>
                ?: throw IllegalArgumentException(
                    "multisig custom.Propose.instructions must be an array",
                )
            require(embedded.size == expectedInstructions.size) {
                "multisig custom.Propose.instructions changed instruction count"
            }
            embedded.forEachIndexed { index, value ->
                requireExactString(
                    value,
                    Base64.getEncoder().encodeToString(expectedInstructions[index]),
                    "multisig custom.Propose.instructions[$index]",
                )
            }
        }

        private fun verifyMultisigApproveJson(
            json: String,
            expectedAccount: String,
            expectedHashLiteral: String,
        ) {
            val root = requireJsonObject(JsonParser.parse(json), "multisig custom")
            requireExactKeys(root, setOf("Approve"), "multisig custom")
            val approve = requireJsonObject(root["Approve"], "multisig custom.Approve")
            requireExactKeys(
                approve,
                setOf("account", "instructions_hash"),
                "multisig custom.Approve",
            )
            requireExactString(
                approve["account"],
                expectedAccount,
                "multisig custom.Approve.account",
            )
            requireExactString(
                approve["instructions_hash"],
                expectedHashLiteral,
                "multisig custom.Approve.instructions_hash",
            )
        }

        @Suppress("UNCHECKED_CAST")
        private fun requireJsonObject(value: Any?, field: String): Map<String, Any?> {
            require(value is Map<*, *>) { "$field must be an object" }
            require(value.keys.all { it is String }) { "$field keys must be strings" }
            return value as Map<String, Any?>
        }

        private fun requireExactKeys(
            value: Map<String, Any?>,
            expected: Set<String>,
            field: String,
        ) {
            require(value.keys == expected) { "$field has unexpected fields" }
        }

        private fun requireExactString(value: Any?, expected: String, field: String) {
            require(value is String && value == expected) { "$field changed" }
        }

        private fun canonicalLowerHex32(value: String, field: String): String {
            val normalized = value.trim().lowercase()
            require(normalized.length == 64 && normalized.all { it in '0'..'9' || it in 'a'..'f' }) {
                "$field must contain 64 hexadecimal characters"
            }
            return normalized
        }

        private fun hasHeader(encoded: ByteArray): Boolean {
            if (encoded.size < NoritoHeader.HEADER_LENGTH) return false
            return encoded[0] == 'N'.code.toByte()
                && encoded[1] == 'R'.code.toByte()
                && encoded[2] == 'T'.code.toByte()
                && encoded[3] == '0'.code.toByte()
        }

        private const val LOG_WIRE_NAME = "iroha.log"
        private const val TRACE_LEVEL_TAG = 0L
        private const val VALIDATION_FEE_MULTISIG_MARKER_PREFIX =
            "iroha:validation_fee:multisig:v1:"
        private const val VALIDATION_FEE_MULTISIG_RESERVED_PREFIX =
            "iroha:validation_fee:multisig:"
    }
}
